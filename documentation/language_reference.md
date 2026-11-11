# Alúma Language Reference — First Specification

The language is called Alúma. Source files use the extension `.alm`. Status: first specification. Sections 1–11 define the language; section 12 records the decisions made and the features deferred to later versions.

## 1. Goals

- Algol/C-level ease of use, with a grammar small enough to read on one page and parse with one token of lookahead.
- Garbage collected. No pointer arithmetic, no explicit freeing, no undefined behaviour.
- Strict at run time: arithmetic overflow is an error, every variable is zero-initialised, every index is checked.
- One glyph, one meaning.
- Exact decimal arithmetic built in (COBOL-style `fixed`).
- Concurrency through isolated tasks that run in lockstep rounds: deterministic, with no data races.

Excluded on purpose: first-class functions (procedures may be passed as arguments but not stored or returned), `for` loops, `goto`, growable arrays in the core language (a library concern), addressable pointers, mutable global variables.

## 2. Lexical structure

**Identifiers.** `letter { letter | digit }`, case-sensitive, no underscores, and not a keyword. Within a module, all declared names (constants, enumeration names, types, variables, procedures, exceptions, imported modules) share one namespace (see 4.1).

**Source text.** Source files are UTF-8. Identifiers use only the ASCII letters A–Z and a–z and the digits. Any Unicode text may appear in comments and in char and string literals. `std` names the standard library (4.1) and cannot be declared.

**Keywords (reserved).**

```keywords
also and array begin bool catch char const defer div do else elsif end enum exception exit false fixed generic if import inbox int is mailbox mod new nil next not of or private proc publisher raise record ref reply return rounded share task then true try type var view void while
```

**Literals.**

| Kind | Form | Example |
|---|---|---|
| integer | digits, or `0x` and hexadecimal digits | `42`, `0xFF` |
| fixed | digits `v` digits | `12v50` (value 12.50, scale 2) |
| char | `'` one character or escape `'` | `'a'`, `'\n'` |
| string | `"` characters and escapes `"` | `"abc\n"` (a new, mutable `String` each time it is evaluated; see 3.4) |
| boolean | `true`, `false` | |
| reference | `nil` | |

**Integer literals.** A literal larger than the largest `int` is a compile error, except that 9223372036854775808 (and 0x8000000000000000) is accepted as the operand of `_`, which gives the smallest `int`.

**Escapes.** Inside a char or string literal, `\n` is a line feed, `\t` a tab, `\r` a carriage return, `\0` the code point 0, `\\` a backslash, `\'` a single quote, `\"` a double quote, and `\u{h}` the code point whose hexadecimal value is `h` (one to six digits, a valid code point). Any other escape is a compile error. A literal cannot contain a raw line break, and a char literal denotes exactly one code point.

The scale of a fixed literal is the number of digits after `v`, so `0v05` is 0.05 with scale 2 and `5v0` is 5.0 with scale 1.

**Comments.** `#` starts a comment that runs to the end of the line. A `#` inside a char or string literal is an ordinary character.

**Glyphs.** Each glyph has one meaning.

| Glyph | Meaning |
|---|---|
| `:=` | assignment |
| `:` | "has type" (variables, parameters, fields, return types) |
| `is` | "is defined as" (constants and types) |
| `=` | value equality |
| `#` | starts a comment (to end of line) |
| `<` `<=` `>` `>=` | ordering |
| `~` | reference identity (same object) |
| `+` `-` `*` | add, subtract, multiply |
| `_` | negation (prefix) |
| `/` | fixed-point division |
| `div` `mod` | integer division, remainder |
| `.` | field selection |
| `::` | module qualification (`Module::name`) |
| `v` | decimal point inside a fixed literal |
| `\` | starts an escape inside a char or string literal |

Parentheses and square brackets are treated as delimiters, not operators.

There is no not-equal or not-identical glyph. `not` binds looser than the comparison operators, so `not a = b` means `not (a = b)` and `not a ~ b` means `not (a ~ b)`.

## 3. Types

### 3.1 Predefined

| Type | Meaning |
|---|---|
| `int` | 64-bit signed integer |
| `bool` | `true` or `false` |
| `char` | a Unicode code point (32-bit); `len` and indexing count code points |
| `fixed(p, s)` | exact decimal, see 7.4 |

### 3.2 Constructed

| Form | Meaning |
|---|---|
| `array [n] of T` | fixed-length array stored inline; `n` is a compile-time constant |
| `record ... end` | named fields |
| `ref R` | reference to a record `R` (a named record type) |
| `ref array of T` | reference to a run-time-sized array |
| `inbox of R` | the receiving end of a task's queue; `R` is a record type (see 8.2) |
| `mailbox of R` | a link: the sending end of a task's queue, which a task receives only as a `spawn` argument; `R` is a record type (see 8.2, 8.10) |
| `reply of R` | an address that takes one answer, or a stated number of answers; `R` is a record type (see 8.2) |
| `task of R`, `task` | the handle `spawn` returns for a task with an inbox of `R`, or without an inbox (see 8.10) |
| `publisher of R`, `view of R` | the owner's end and the shared read end of published state; `R` is a record type (see 8.12) |

References never point to `int`, `bool`, `char` or `fixed` values. Named types are introduced with `type T is ...`, and a `ref` may name a record type still being declared, which allows recursive structures.

### 3.3 Zero values

Every variable, field and array element starts as: `int` 0, `bool` false, `char` code 0, `fixed` 0, `ref`, `mailbox`, `reply`, `view`, `publisher`, `task`, `File` and `Listener` `nil`, and aggregates zero in every part.

### 3.4 Strings

Strings are `char` arrays. The standard library declares `type String is ref array of char;`.

### 3.5 Copy and sharing

Assignment of `int`, `bool`, `char`, `fixed`, records and fixed arrays copies the value. Assignment of a `ref` copies the reference, so both names refer to the same object.

Type compatibility is by name: two types are the same only if they are the same declared type (or both spell out the same predefined type). Every `record` declaration is its own type. Every `ref` may be `nil`.

## 4. Declarations

```
const Size is 100;
enum  Get, Put, Delete;
type  Node is record key: int; left: ref Node; right: ref Node; end;
exception NotFound;
proc  name(params): result block;
```

- Constants are compile-time constant expressions, usable wherever a size is needed (`array [Size] of int`, `fixed(Digits, 2)`).
- `enum A, B, C;` declares the `int` constants 0, 1 and 2 in order. They have no type of their own, there is no iteration over them and no completeness checking, and they can be used wherever a compile-time constant is allowed.
- A procedure with a result type is a function.
- Procedures are declared at top level only; there are no nested procedures.
- Variables are declared only at the start of a procedure body. There are no top-level variables, so all mutable state belongs to a task (see 8). Constants, enumerations, types, exceptions, shared values (4.2) and procedures are declared at top level. A name must be declared before use, except among the members of a group (4.3). A type may refer to itself through `ref`, and a procedure may call itself.

### 4.1 Modules and visibility

- A module is a source file. Its name is the file name without the extension, and must be an identifier that is not a keyword. There is no module header in the source.
- The program is the main module together with every module it imports, directly or through others, and all of them are compiled together from source. There are no interface files. The compiler is given the main module, which must declare `start` (9.1). No other module may declare `start`. The module `A` is the file `A.alm`, searched for in the directories of the `--path` option (8.9); the first match wins. Failing that, the implementation's own modules are searched. `Net` (9.7) is one of them, and a program's own `Net.alm` on the path takes its place.
- `import A, B;` appears at the top of a file, before any other declaration. (An instance of a generic module can also be imported later in the file, 4.4.) It makes the visible names of those modules available as `A::name`. Imports are not transitive: if `A` imports `B`, a module that imports `A` must also import `B` to use `B`'s names. A module cannot import itself, and the imports must not form a cycle (a compile error that names the cycle).
- Each module has its own namespace. The standard library is the module `std`, which every module imports automatically; no `import` line is needed. Library names can be written unqualified, as in `len(a)`, or qualified, as in `std::len(a)`.
- **Hiding library names.** A declaration at the top level of a module hides a library name of the same spelling in that module, wherever in the module it is used. A use of the name before its declaration is an error, not a use of the library name. `std::name` always reaches the library one. The names `len` and the task, handle and view operations of section 8 (`spawn`, `send`, `receive`, `await`, `sync`, `self`, `replyTo`, `answer`, `link`, `cancel`, `awaitEnd`, `ended`, `failed`, `closing`, `disown`, `newView`, `publish`, `latest`, `version` and `awaitChange`), which the language itself relies on, cannot be hidden. A parameter or a local variable hides a library name of the same spelling in the same way, inside its procedure, and cannot hide the names listed above either. Hiding in one module has no effect on other modules or on the library.
- An imported module's name is entered in the importing module's namespace, so it cannot also be used for something declared there.
- `private` before a constant, enumeration, type, exception, shared value or procedure declaration hides it from other modules (for an enumeration, all its names). Everything else is visible. Using a private name from another module is a compile error. A record field is visible unless its declaration starts with `private`: a private field can be read and assigned only in the module that declares the record type, and a use anywhere else is a compile error. Other modules can still hold, pass, copy and compare whole records with private fields, and `new` and variables of the type give zeroed records. A private type may appear in the signature of a visible procedure: other modules can pass its values around but cannot name the type.
- A qualified name (`A::name`, or `std::name` for the library) denotes a constant, an enumeration name, a type, an exception, a shared value or a procedure, written in any place an unqualified one is allowed. Modules hold no variables (see 4), so a qualified name is never assigned to. Qualification has one level only: there is no `A::B::c`.
- Declarations belong to their module. Two modules may each declare `Node` or `Empty`. By name equivalence (7.1) `A::Node` and `B::Node` are different types, and `A::Empty` and `B::Empty` are different exceptions.
- Modules exist only at compile time. A task body may be a qualified procedure, as in `spawn(Worker::run, ...)`, and message types may come from any module.

### 4.2 Shared values

A shared value is read-only data that every task can read without copying it.

```
share Squares: ref array of int is squares(1000);
```

- `share Name: T is expr;` is a top-level declaration. `T` is the type of the value, and `expr` computes it. It may be preceded by `private`, and from another module it is written `Module::Name`.
- `T` must be a type that a message may carry (8.2) and that may be published (8.12): it may not contain an `inbox`, `mailbox`, `reply`, `view`, `publisher`, `task`, `File` or `Listener`.
- **Initialization.** Before round 1, the shared values are built one at a time. Modules are visited depth first in the order of their `import` declarations, each once, starting from the main module, and a module's own shared values are built from the top down. An instance import among the declarations (4.4) is visited at its position, after the shared values above it and before those below it. An initializer can use any shared value built earlier, including those of imported modules.
- Each initializer runs in a pseudo-task of its own, numbered 0, with a private heap. Its result is then copied into the shared region the way a message is copied (8.2): substructure that is reachable by several paths is copied once and cycles are preserved. The copy is marked read-only.
- An initializer can call any procedure. It can also call `args()`, `now()`, `random(n)` and the reading operations (`open` for reading, `readLine`, `readBytes`, `close`, `stdin`, `exists`, `list`), which complete immediately and are recorded for replay (8.7). Writes (including standard output), `open` for writing, `delete`, `rename`, `mkdir`, `sleep`, `sleepRounds`, every operation of `Net` (9.7), and the task, handle and view operations (`spawn`, `send`, `receive`, `await`, `sync`, `self`, `replyTo`, `answer`, `link`, `cancel`, `awaitEnd`, `disown`, `newView`, `publish` and `awaitChange`) raise `BadContext`.
- An initializer is limited by the step budget, the memory limit and the shared-region limit, all of them compiler options (8.9). If an exception leaves an initializer, or a limit is exceeded, the program ends with status 1 before round 1, and the report gives `(initialization of Module::Name)` in place of the task number.
- **Reading.** A shared value can be read by every task at no cost. A reference into the shared region can be stored, passed, compared with `~` and sent in a message, and when sent it is copied as a reference, by identity, and not as the structure it points to.
- **Writing.** Every object in the shared region is read-only, and a write to a field or an element of one raises `ReadOnly`. There is no static check: a reference to a shared object has the same type as any other reference, so it can be passed to any procedure. Writes to other objects are unaffected.
- A shared name is not a variable and cannot be assigned to. If its type is a value type (an `int`, a record, a fixed-length array), an element or field of it cannot be assigned either, and passing the whole value to a by-reference parameter copies it into a temporary, as for any non-variable argument (7.2).
- The shared region lives as long as the program, is never collected, and is not counted in any task's live size (8.4). It points only to itself.

### 4.3 Recursive groups

Types that refer to each other, or procedures that call each other, are declared as a group.

```
type Tree is record
  label: int;
  kids: ref Forest;
end;

also type Forest is record
  trees: ref array of ref Tree;
end;
```

- A declaration that starts with `also` joins the group of the declaration just before it. A group is a declaration that does not start with `also`, followed by all the `also` declarations after it.
- All members of a group are types, or all are procedures. A group member may start with `also private`. `also` cannot come first in a file, after any other kind of declaration, or after a declaration of the other kind.
- Inside a group, every member name can be used in every member, wherever the member is written, including in signatures and in the types of fields. After the last member, the names are in scope as usual.
- Member names must differ from each other.
- A type group cannot contain a cycle of containment by value: a record or fixed-length array may hold another member only through `ref`, unless the other member does not hold it back. An alias loop, such as `type A is B; also type B is A;`, is also an error. Both are compile errors that name the cycle.
- A group changes nothing at run time.

### 4.4 Generic modules

A generic module is written once and used with different types, constants and procedures.

```
generic type T;
```

- A module is generic when its file starts with `generic` lines. Each line declares one parameter:
  - `generic type T;` a type;
  - `generic const N: int;` a constant `int`;
  - `generic proc name(params): result;` a procedure with that signature (the parameter names are only for the reader).
- The `generic` lines come first in the file, before every `import` and declaration. A generic module cannot be the main module and cannot declare `start`.
- A generic module is used with `import Name is Generic(arguments);`, which makes an **instance** called `Name` in the importing module. `import` can list plain modules and instances together: `import Lists, Names is Table(String, int, stringEqual);`. The arguments are in the order of the `generic` lines: a type for `generic type`, a constant expression for `generic const`, and the name of a procedure (qualified if needed) for `generic proc`. An argument that starts with an identifier is looked up as a name and must fit the parameter.
- A procedure argument must have exactly the parameter types and result type of the declared signature, with the type parameters replaced by the given types. The built-in procedures that accept several types (9) cannot be given as arguments.
- The visible names of the instance are written `Name::member`. A generic module cannot be imported without arguments, and a module without `generic` lines cannot be given any.
- **Where an instance is imported.** The `import` line at the top of the file can list instances, as above. An instance can also be imported on a line of its own among the top-level declarations: `import Name is Generic(arguments);`. Its arguments can then name types, constants and procedures declared above it in the same file, and `Name` is declared at that line and can be used after it, like any other declaration. The line takes neither `private` nor `also`, and an `also` declaration cannot follow it. Imports of plain modules stay at the top of the file (4.1).
- **One instance per program.** The same generic module with the same arguments is one instance, wherever it is imported. Two arguments are the same when they are the same type, constants of equal value, or the same procedure declaration. The import name is only a local name for it. Different arguments give different instances, with different types and exceptions.
- **Opaque type parameters.** In a generic module, a value whose type is a type parameter `T` can be assigned, passed, stored in fields and array elements, and made as a zero value (`new(array of T, n)`). It cannot be compared with `=`, ordered, used in arithmetic, compared with `nil`, indexed or have fields selected. `=` is also rejected on records and arrays inside the module when any part of them has type `T`. Any other operation comes from the module's `generic proc` parameters. The module is checked once, not once per instance.
- Constant parameters are used wherever a constant is allowed: `array [N] of T`, `fixed(N, 2)`.
- A procedure parameter is called like any procedure and can be passed to other procedures. It cannot be assigned or stored (7.2).
- Where a type that contains `T` is used as a message type or as the type of a shared value, the given type is checked at the import (none of the types that a message or a shared value may not contain, 8.2 and 4.2), and the error names the import.
- A generic module can import modules and instantiate other generic modules, giving its own parameters as arguments. Instantiations can nest at most 32 deep. A chain that goes deeper is a compile error that names the chain.
- The types, exceptions, shared values and procedures of an instance exist once per instance.

## 5. Statements

- **Assignment** `x := e`, or `x := rounded e` (see 7.4).
- **Call** `p(args)`, or `void f(args)` to call a function and discard its result. A function's result is never discarded implicitly: calling a function as a plain statement is an error, and `void` is an error on a procedure that returns nothing.
- **If** `if c then ... { elsif c then ... } [ else ... ] end`.
- **While** `while c do ... end`.
- **Exit and next** `exit;` leaves the innermost `while`, and `next;` skips the rest of its body and evaluates its condition again. Both are errors outside a `while` and do not cross a procedure boundary. Leaving a `try` this way is not an exception.
- **Defer** `defer p(args);` or `defer void f(args);` schedules a call for when the procedure exits (5.1).
- **Return** `return [e]`.
- **Raise** `raise Name`, or a bare `raise` inside a handler to re-raise the exception being handled.
- **Try** `try ... catch Name do ... [ catch do ... ] end`. A handler may list several names, `catch A, B do`, and runs for any of them. A handler with no name catches anything and must come last. An exception no handler matches propagates to the caller; if it reaches the top, the program stops as described in 7.5.

Every statement in a list is terminated by `;`.

### 5.1 Deferred calls

- `defer p(args);` (or `defer void f(args);` for a function) schedules a call of `p` to run when the procedure that contains the `defer` statement exits: by `return`, by reaching its end, or because an exception leaves it. A task body is a procedure, so this holds for a task too.
- When the `defer` statement executes, the call is recorded. Arguments that are not variables are evaluated then, into temporaries. Arguments that are variables are bound by reference, as in any call, so the call sees their values at the time it runs. Objects the recorded call refers to stay reachable until it has run.
- When the procedure exits, its recorded calls run in the reverse order of recording. A `return e` computes its result first. The deferred calls then run, and then control goes back to the caller.
- A `defer` statement may appear anywhere in a procedure body except inside a `while`. A procedure therefore records at most as many calls as it has `defer` statements. `exit` and `next` leave a loop, not the procedure, so they run no deferred calls.
- If a deferred call raises an exception, the remaining deferred calls still run, and the procedure exits with the exception from the last call that failed, in place of any exception already on its way out.
- A task's own deferred calls, those of its body procedure, run when it ends, after everything it started has ended (8.11). A task that is stopped by the backstop runs none of them. A deferred call that raises while its task is being closed replaces `Closed` (8.6).
- Cost: the `defer` statement is 1 step (8.4). A deferred call costs what an ordinary call costs, when it runs.

## 6. Expressions

Precedence from loosest to tightest:

| Level | Operators |
|---|---|
| 1 | `or` |
| 2 | `and` |
| 3 | `not` (prefix, may repeat) |
| 4 | `=` `<` `<=` `>` `>=` `~` (non-associative: one per expression) |
| 5 | `+` `-` |
| 6 | `*` `/` `div` `mod` |
| 7 | `_` (prefix, may repeat) |
| 8 | literals, designators, calls (with optional selectors), `new`, parentheses |

There is no "not equal" or "not identical" operator: write `not (a = b)` or `not (a ~ b)`. The parentheses are optional, because `not` binds looser than the comparisons.

`and` and `or` short-circuit: the right operand is evaluated only if the left does not already decide the result, so `(j >= 0) and less(x, a[j])` never evaluates `a[j]` when `j` is negative. Both are evaluated left to right.

In general, evaluation order is strictly left to right: the operands of an operator, the arguments of a call, and, in an assignment, the index expressions of the target before the right-hand side.

**Selecting from a call.** A call can be followed by index and field selectors: `args()[0]`, `People::get(people, "ann").age`. The selectors apply to the result. A reference result is followed, so a `nil` result raises `NilReference` and an index out of range raises `IndexRange`. A record or fixed-length array result is read from the temporary that holds it. Such a selection is an expression only: it cannot be the target of `:=`, and as an argument it is a temporary (7.2).

## 7. Semantics

### 7.1 Equality and identity

- `=` compares values. It is defined on `int`, `bool`, `char`, `fixed`, and on records and arrays that contain no references, mailboxes, replies, views, publishers, tasks or files. It is not defined on those types.
- `~` is defined only on references, mailboxes and views (and `nil`). It is true when both sides are the same object.
- Comparing contents of reference data (for example strings) is done with library procedures such as `stringcmp`.

### 7.2 Parameters

All parameters are passed by reference. A parameter is an alias for the caller's variable, so assigning to a parameter changes the caller's variable. An argument that is not a variable (a literal or an expression) is evaluated into a temporary, and the temporary is what is aliased. To avoid disturbing the caller, copy a reference parameter into a local before walking it, as the search example does.

Passing the same variable, or overlapping array elements, for two parameters is allowed. The parameters are true aliases: they denote the same storage, so assignments through them take effect in program order and the result is the same in every implementation. A temporary made for a non-variable argument (above) is a separate variable, never an alias of another argument.

**Procedure parameters.** A parameter may itself be a procedure, declared with a procedure type such as `less: proc(int, int): bool`. The argument is the name of a procedure whose parameter and result types match exactly. A procedure parameter is called like any procedure, but it cannot be assigned, stored in a variable or field, or returned, which keeps procedures second-class. It can be passed on to another procedure.

### 7.3 Integers

`int` is 64-bit signed. A result outside the range, in either direction, raises `Overflow`. `div` or `mod` by zero raises `DivideByZero`.

`div` and `mod` follow the number-theoretic (Euclidean) definition: for `b` other than 0, `a = (a div b) * b + (a mod b)` and `0 <= a mod b < |b|`, so the remainder is never negative. Examples: `_7 div 2 = _4` and `_7 mod 2 = 1`; `7 div _2 = _3` and `7 mod _2 = 1`; `_7 div _2 = 4` and `_7 mod _2 = 1`. The one overflowing case is the smallest `int` divided by `_1`.

### 7.4 The `fixed(p, s)` type

`fixed(p, s)` is an exact decimal number with at most `p` digits in total, of which `s` follow the decimal point. For example, `fixed(7,2)` holds -99999.99 to 99999.99 in steps of 0.01. It is stored as an arbitrary-precision integer scaled by 10ˢ, so `0v10 + 0v20 = 0v30` exactly. `p` may be any positive constant with no upper limit, and `s` must satisfy 0 <= s <= p. Intermediate results of `+`, `-` and `*` are exact however many digits they need; only an assignment to a variable of a declared `fixed(p, s)` type is range-checked.

- Addition and subtraction are exact, with the larger of the two scales.
- Multiplication is exact, and the scales add.
- Comparison is by numeric value, so `1v5 = 1v50`.
- An `int` acts as a `fixed` of scale 0 when mixed with fixed values. `/` with two `int` operands is a compile error; use `div`.
- Division `/` gives the quotient at the sum of the two scales, as multiplication does: a scale-2 value divided by a scale-2 value has scale 4, and dividing by an `int` keeps the dividend's scale. If the quotient is not exact at that scale, non-zero digits are lost, and this is a rounding step (below). A zero divisor raises `DivideByZero`. For example, `6v00 / 2v00` is `3v0000`, `1v00 / 4v00` is `0v2500`, and `1v00 / 3v00` raises `LostPrecision` without `rounded` and gives `0v3333` with it.
- **Assignment is where precision is checked.** Too many integer digits raises `Overflow`. If non-zero fractional digits would be lost, `LostPrecision` is raised, unless the statement says `rounded`, which rounds half away from zero. Digits that are not lost cause no error. The same rule applies when a `fixed` is assigned to an `int`.
- **Explicit scale.** `divide(a, b, k)` and `rescale(x, k)` give an exact `fixed` of scale `k`, a constant expression of 0 or more. `divide` is the exact quotient of `a` and `b` rounded once to scale `k`, whatever the scales of `a` and `b` are. `rescale` changes the scale of `x` (an `int` counts as scale 0) and fills in zeros when `k` is larger. A zero divisor in `divide` raises `DivideByZero`. For example, `divide(1v0, 8v0, 3)` is `0v125`.
- **Rounding modes.** `divide` takes a fourth argument and `rescale` a third: a constant naming a mode. The modes are `RoundHalfAway` (nearest, ties away from zero), `RoundHalfEven` (nearest, ties to the even digit), `RoundDown` (toward zero), `RoundUp` (away from zero), `RoundFloor` (toward minus infinity) and `RoundCeiling` (toward plus infinity). A call with a mode rounds as the mode says, never raises `LostPrecision`, and is not affected by `rounded`. `rescale(2v675, 2, RoundHalfEven)` is `2v68`, `rescale(_2v5, 0, RoundFloor)` is `_3`, and `divide(1v00, 3v00, 4, RoundHalfAway)` is `0v3333`.
- **Rounding steps** are the places where digits can be lost: a division, a call of `strFixed` (9.2), a call of `divide` or `rescale` without a mode, and the final assignment. Without `rounded`, losing a non-zero digit at any of them raises `LostPrecision`. With `rounded`, each rounds half away from zero.
- **Single rounding.** If the right-hand side of a `rounded` assignment is a single division, or a single `strFixed`, `divide` or `rescale` call without a mode (possibly in parentheses), it is rounded once, to the smaller of its own scale and the destination's scale, so the assignment loses nothing more. Elsewhere in an expression, each division and `strFixed` rounds at its own scale, and the assignment then rounds again, which can differ from rounding once. To avoid that, split the expression into several statements.
- **Named number types.** `type D is fixed(p, s);` and `type D is int;` make named types. A value whose type is a named type has that name as its *tag*. Plain `int` and `fixed` values, and literals, have no tag.
  - `+`, `-` and the comparisons accept two values with the same tag, or one tagged and one untagged value. Two different tags are a compile error. The result of `+` or `-` has the tag, if there is one.
  - Every other operation (`*`, `/`, `div`, `mod`, `divide`, `rescale`, `floor` and so on) accepts any tags and gives an untagged result.
  - An untagged value, or a value with the same tag, can be assigned to a variable of the named type, with the checks above. A tagged value can be assigned to an untagged variable. A tagged value cannot be assigned to a variable with a different tag.
  - An argument that is a variable must have exactly the type of its parameter (7.2). Any other argument follows the assignment rule.
- There are no conversion procedures. An `int` mixes with `fixed` values as a scale-0 `fixed`, and a `fixed` is assigned to an `int` under the assignment rule above.

### 7.5 Exceptions

Predefined exceptions:

| Exception | Raised when |
|---|---|
| `Overflow` | an `int` or `fixed` result is out of range, in either direction |
| `LostPrecision` | non-zero fractional digits would be lost in an assignment without `rounded` |
| `DivideByZero` | `div`, `mod` or `/` has a zero divisor |
| `IndexRange` | an array index is below 0 or at least `len` |
| `NegativeSize` | a size, count or shift that must not be negative is negative: `new(array of T, n)`, `repeat`, `power`, `shl`, `shr`, `sleep`, `sleepRounds`, and `random(n)` with `n` less than 1 |
| `NilReference` | a field or element is accessed through `nil` |
| `OutOfMemory` | a task's live size would exceed the memory limit, or a sent message does not fit in the receiver's limit (see 8.2, 8.5, 8.12) |
| `OutOfSteps` | with `--strict-steps`, a task is charged a step after its budget for the round is used up (see 8.4) |
| `StackOverflow` | procedure calls nest deeper than the fixed limit (see 8.5) |
| `DeadTask` | a message is sent to a task that ended in an earlier round; a reply is used a second time; a `spawn` argument names a task that is marked or has ended; `awaitChange` is called on a view whose owner has ended, with nothing newer (see 8.2, 8.10, 8.12) |
| `Deadlock` | every live task is waiting and nothing can wake any of them (see 8.6) |
| `Closed` | a task that is closing reaches an interruption point, or a marked task starts new work (see 8.11) |
| `Timeout` | a read, accept or connect given a time limit does not complete in time, or a closing task has used up its grace time (see 9.3, 9.7, 8.11) |
| `InvalidCharacter` | `chr(n)` is given a value that is not a Unicode code point |
| `FormatError` | text is not in the form a library procedure requires (a number, or a non-empty separator) |
| `BadEncoding` | bytes read as text are not valid UTF-8, or text written contains a value that is not a Unicode code point |
| `FileNotFound` | `open`, `delete` or `list` names a file or directory that does not exist |
| `PermissionDenied` | the operating system refuses access to a file |
| `ReadOnly` | a field or element of a shared object is assigned (see 4.2) |
| `BadContext` | an operation that is not allowed while shared values are built is called (see 4.2) |
| `AssertFailed` | `assert(c)` is called with `c` false |
| `IOError` | any other file or stream failure, including use of a closed file and a deferred write failure (see 9.3) |

Programs declare their own with `exception Name;`. Exceptions carry no data. There is no `finally`: to release a resource such as a file, record the clean-up call with `defer` (5.1). To react to a failure, catch it, and re-raise with a bare `raise` if needed.

**Uncaught exceptions.** If an exception propagates out of the initial task, the program writes the following to standard error and exits with status 1:

```
uncaught Overflow in Stats::sum (task 1)
  evaluating: total + x
  called from: Stats::average
  called from: App::start
```

The first line gives the exception, the procedure that raised it and the task it was raised in. The second gives the source text of the expression or statement being evaluated at that point (for an explicit `raise`, the `raise` statement). The remaining lines give the calling procedures, innermost first. Procedures and declared exceptions are written with their module name (`IntList::Empty`); predefined exceptions are not. For an exception during initialization (4.2), the first line ends with `(initialization of Module::Name)` in place of the task number.

**Failed tasks.** When any other task fails, the same report is written at the boundary at which the task ends, with `failed` in place of `uncaught` and a last line `  parent: task N`, whether or not the parent will see the failure. How the failure is delivered, and when it makes the exit status 1, is described in 8.6.

### 7.6 Memory

Objects are created with `new` and reclaimed by the garbage collector when unreachable. Each task has its own heap (see 8). `new(R)` creates a zeroed record, and `new(array of T, n)` creates a zeroed array of length `n` (a negative `n` raises `NegativeSize`). Array indexing starts at 0 and is bounds-checked.

## 8. Tasks

Concurrency is provided by tasks that run in lockstep rounds. Each task has a private heap. Tasks communicate by messages, which are copies delivered between rounds, and share changing state only through views, which are read-only snapshots (8.12). No memory that can change is reachable from two tasks, delivery happens only at round boundaries, and every limit is counted in units that depend only on the program and its input, so a program's behaviour does not depend on timing or on the number of processors.

A task reaches another task through one of three addresses. A **link** is a `mailbox`, which a task receives only when it is spawned (8.10). A **reply** is an address for one answer, or a stated number of them, carried in a request (8.2). A **view** is published state that any holder can read (8.12). Only links make one task wait for another when tasks are closed (8.11).

### 8.1 Tasks and heaps

- A task is an independent line of execution with its own heap and call stack. Every object a task creates lives in its heap, and no reference points from one heap into another, except references into the shared region (4.2) and into published versions (8.12), which are read-only.
- There are no mutable global variables (see 4). All mutable state belongs to a task.
- The program starts with one task, the initial task, number 1, which runs the program's entry procedure `start` (see 9.1). Others are created with `spawn`. Numbers are assigned at the boundary at the end of the round in which tasks are created (8.3), ordered by the creator's number and then by creation order. A task's number is therefore always greater than its parent's.
- A task body is a procedure. If its first parameter has type `inbox of R`, where `R` is a record type, the task has an inbox and the parameter is bound to it. Otherwise the task has no inbox.
- `spawn(body, args...)` starts a task running `body` and returns a handle to it (8.10): a `task of R` if the body has an inbox of `R`, and a `task` if it has none. The remaining parameters are bound to the arguments. The new task starts in the round after the one in which it was spawned.
- **Arguments.** Plain data is copied as a message is copied (8.2), and `reply` and `view` values are copied by identity. A parameter of type `mailbox`, `File`, `Listener` or `publisher` must be given an argument of exactly that type, never a record that contains one. A `mailbox` argument names a task and makes the new task a holder of it (8.10). A `File`, `Listener` or `publisher` argument must be a variable. It is **moved**: the new task owns the resource, and when the call succeeds the variable becomes `nil`. If `spawn` raises an exception, nothing is moved. An `inbox` or a `task` cannot be an argument.
- Every task except the initial one has a **parent**, the task that spawned it. The parent of the initial task is the runtime. A task cannot outlive its parent (8.11).
- A task **finishes** when its body returns or an exception leaves it. It **ends** when everything it started has ended and its outermost deferred calls have run (8.11). Its heap is released at the end of the round in which it ends.

### 8.2 Mailboxes, replies and messages

- `inbox of R` is the receiving end of a task's queue and can be used only by the task that owns it.
- `mailbox of R` is the sending end of a queue, a **link** (8.10). It can be copied, stored and passed to procedures within a task. It reaches another task only as an argument of `spawn`: it cannot be sent in a message and cannot be part of a shared or published value. The zero value is `nil`, and sending to it raises `NilReference`.
- `self(inbox)` returns a `mailbox of R` for the task's own inbox. A task gives its child a way to reach it by passing `self(queue)` as an argument to `spawn`. Messages carry no implicit sender.
- `reply of R` is an address for answers to a request. `replyTo(queue)` returns a reply to the task's own inbox `queue` that takes one answer, and `replyTo(queue, n)`, with `n` of 1 or more, returns one that takes up to `n`; `n` less than 1 raises `NegativeSize`. A reply can be copied, sent in a message and passed to `spawn`, and every copy refers to the same cell, which counts the answers it has left. `answer(r, m)` sends the message `m` to the task that made the reply, as `send` does, and uses up one of the answers. Once none are left, an `answer` through any copy raises `DeadTask`, and so does an `answer` to a task that has ended. If several answers to one reply are given in a round, they are taken in the order of the answering tasks' numbers and then of sending, until the answers left are used up; the rest are discarded, and their senders' next return from `sync` or `await` raises `DeadTask`. Answers are messages and count against the requester's queue (8.5), so a requester that does not receive them makes its answerer fail with `OutOfMemory`, as with `send`. A reply creates no link and no wait, so replies that run both ways between two tasks do not break the order of waits (8.10). The zero value is `nil`, and `answer` through it raises `NilReference`.
- A message is a reference to a record of type `R`. The compiler rejects an `R` whose fields, directly or through references, contain an `inbox`, `mailbox`, `publisher`, `task`, `File` or `Listener`. A message may contain `reply` and `view` values.
- `send(mb, m)` takes a `mailbox of R` or a `task of R`. It copies the objects reachable from `m` at the moment of the call, so later changes by the sender do not affect the message. An object reachable by several paths is copied once, and cycles are preserved. `reply` and `view` values, and references into the shared region (4.2) or into a published version (8.12), are copied by identity. Messages are always copied; no operation moves an object to another task.
- A sent message becomes receivable at the start of the next round. Sending to a task that ended in an earlier round raises `DeadTask`. Sending to a task that has not ended, or that ends in the current round, succeeds, and a message that is still queued when a task ends is discarded.
- Queued messages count toward the receiving task's memory limit (8.5). When the messages sent in a round are appended to a queue (8.3) and one does not fit, it is discarded and its sender's next return from `sync` or `await` raises `OutOfMemory`. The uncaught-exception report names the `send` that caused it.
- `receive(inbox, m)` removes the oldest receivable message, assigns it to the variable `m` and returns true. If there is none, it leaves `m` unchanged and returns false. It never waits.
- `await(inbox, m)` does the same, but if no message is receivable it blocks the task until the first round in which one is, and then assigns it. `await` is an interruption point (8.11).

### 8.3 Rounds

- Execution proceeds in numbered rounds, starting at 1. At any time a task is runnable, blocked, or ended. A task blocks in `await`, `awaitChange`, `awaitEnd`, `sleep`, or in an operation that waits for an outside event (9.2, 9.3, 9.7). In each round every runnable task runs, from its start or from where it last stopped, until it calls `sync()`, uses up its step budget, blocks, or ends. Within a round, tasks may run in any order or at the same time, and the outcome is identical.
- `sync()` stops the caller for the rest of the round. It stays runnable and resumes in the next round. `sync` is an interruption point (8.11).
- A blocked task is not run and consumes no work.
- **The round boundary.** When every task has stopped, the round ends and the following happens in order:
  1. Buffered output is written in task-number order, and socket output is offered to the operating system.
  2. Closing tasks whose grace counter has reached `--grace` are stopped (8.11).
  3. Tasks that ended in this round, or were stopped, are recorded as ended. Their queued messages are discarded, the views they owned become ended (8.12), and they are removed from their parents' lists of children.
  4. Tasks created in this round are numbered (8.1) and placed in the tree of tasks.
  5. The messages sent and answers given in this round are appended to their receivers' queues as one stream, ordered by the sender's task number and then by order of sending. A message that does not fit is discarded (8.2).
  6. The versions published in this round become visible, the last one per view (8.12). The memory growth caused by steps 3, 5 and 6 is checked (8.5).
  7. Cancels requested in this round, and signals that take effect at this boundary, are applied: the marked set is closed, blocked `accept`s of marked tasks are interrupted, and every marked task whose waits are over becomes closing (8.11).
  8. The failures of tasks that ended are handled (8.6).
  9. Outside events take effect: input, accepts, connects, expiries of timers and timeouts, and completions of writes. This comes after step 7, so a read that completes at the same boundary as its cancel is dropped (8.11).
  10. The next round begins. A blocked task becomes runnable if it has a receivable message, the round it sleeps until with `sleepRounds` has begun, a view it waits on has changed or ended, a child it waits for has ended, it has a pending `Closed`, or an event it waits for has taken effect.
- Output from a task is buffered until the end of the round. Events from outside the program, such as input becoming available or a sleep expiring, take effect only at round boundaries: a task blocked on one resumes in the first round that starts after its result is available (9.2, 9.6). The round in which an event takes effect depends on timing, so a replay log records that round number with every event (8.7).

### 8.4 Step budget and size units

The step budget `N`, the memory limit and `--strict-steps` are compiler options (8.9) and apply to every task. Their values are part of the program's identity, so a replay must use the same ones.

**Steps.** A task's execution is counted in abstract steps:

| Event | Steps |
|---|---|
| a simple statement (assignment, call, `return`, `raise`, `defer`) | 1 |
| evaluating the condition of `if`, `elsif` or `while` | 1 each time |
| entering a user-defined procedure | 1 |
| unwinding one procedure frame | 1 |
| `fixed` add or subtract | limb count (units) of the larger operand |
| `fixed` multiply or divide | product of the operands' limb counts |
| `new`, `send`, `answer` | the copy price of the objects created or copied |
| `spawn` | 1 + the copy price of the copied arguments |
| `publish` | 1 + the copy price of the graph published |
| `round`, `replyTo`, `link`, `cancel`, `awaitEnd`, `disown`, `ended`, `failed`, `closing`, `newView`, `latest`, `version`, `awaitChange` | 1 each |
| `int` arithmetic, comparison, indexing | free |

The **copy price** of a set of objects is one step for each object plus one step for each 16 units, rounded up. A reference into the shared region or into a version counts as one unit and is not followed.

Condition evaluation costs a step so that an empty `while true do end` still uses its budget. Expressions are otherwise free because their size is fixed by the program text, and anything that loops is charged.

**Budget.** At the start of every round each live task's budget is increased by `N`, capped at `N`, and a task runs in a round only if its budget is positive. When the budget reaches zero the task is stopped as if it had called `sync()`, except that this stop is not an interruption point. An operation cannot be interrupted: if it costs more than the budget left, it completes and the balance goes negative, and the task stays idle until the refreshes bring the balance back above zero. For example, with a full budget `N`, an operation costing 2.5 `N` leaves -1.5 `N`; the next round's refresh gives -0.5 `N` and the task stays idle; the round after that it runs with 0.5 `N`. The cap stops a long-blocked task from banking budget. The balance is kept without a bound.

**Strict steps.** With the option `--strict-steps` the budget is a limit and not a time slice. A task whose balance has reached zero or below raises `OutOfSteps` at the next step it is charged, instead of being stopped, and its balance is then set to `N` so that handlers and deferred calls can run. The balance is also set to `N` at the start of every round, so no debt is carried and a task never idles for it. Tasks then yield only at their own `sync`, `await` and other blocking calls, and a run that raises no `OutOfSteps` gives the same results for every value of `--steps`.

**Size units.** A unit is a count, not bytes, and an implementation may lay objects out however it likes as long as it counts by this table.

| Value | Units |
|---|---|
| `int`, `bool`, `char`, reference, `mailbox`, `reply`, `publisher`, `task`, `File`, `Listener` | 1 |
| `view` | 1, and after its owner has ended, 1 plus the units of its last version (8.12) |
| `fixed(p, s)` | ⌈p / 18⌉ (one unit per 18 digits) |
| record | 1 + the sum of its fields |
| fixed-length array of `n` elements of `T` | n × size(T) |
| `ref array of T` | 1 + n × size(T) |

A task's live size is the sum over distinct objects of its own heap reachable from its roots (objects in the shared region are not counted, and a reference to one is 1 unit), counted once however many references point to them, together with the versions it can reach (8.12). For example, a tree node with a key and two children is 4 units, and a 1000-character string is 1001.

### 8.5 Determinism rules

1. **Memory limit.** A task's live size is the units (8.4) of the distinct objects reachable from its call stack, plus its queued messages, plus the versions it can reach (8.12). `OutOfMemory` is raised exactly when, immediately after an allocation (a `new`, a copy made by `spawn`, a `publish` or a `latest`), the live size would exceed the limit. The size is recounted exactly only when the last known live size plus the units allocated since could exceed the limit, so collector timing is never observable.
   Growth that happens at a round boundary, when a queue receives a message or when the owner of a view ends (8.12), is added to the units allocated since. If the sum could exceed the limit, the task is recounted exactly at the boundary, and if its live size does exceed the limit its next return from `sync` or `await` raises `OutOfMemory`.
2. **Call depth.** `StackOverflow` is raised when procedure calls nest deeper than a fixed number of levels, counted in calls and not in machine stack bytes. The number is a compiler option, like the other limits (8.9).
3. **No identity leaks.** No operation reveals an object's address, an identity number, or a hash of identity. Comparing two references for identity is allowed, because it yields only true or false. Library structures iterate in insertion order, never in hash or address order.
4. **No collector hooks.** There are no finalizers, no weak references, and no queries of heap size or collection state.
5. **Time, randomness and the network** enter a program only through `now`, `sleep`, `random` (9.6) and the operations of `Net` (9.7), whose results are outside events recorded for replay.

### 8.6 Failure and termination

- **A failing task.** An exception that leaves a task body makes the task fail. An exception named `Closed` that leaves the body is not a failure (8.11). When a failed task has ended, its failure is handled at step 8 of the boundary:
  - It is written to standard error in the form of the uncaught-exception report (7.5), with `failed` in place of `uncaught` and a last line `  parent: task N`. This happens for every failure, whatever happens to it next.
  - If the parent is not marked, the child has not been disowned and the parent has no pending failure, the failure becomes the parent's **pending failure**. If several children of one parent fail at the same boundary, the lowest-numbered one takes that place. Any other failure is not raised.
- **Delivery.** A pending failure is raised from the parent's next `sync`, `await` or `awaitChange`, and the parent then has none. `awaitEnd(h)` raises the failure of `h` if it ended by failure and was not disowned. When `failed(h)` or `awaitEnd(h)` observes a failure that is pending, it ceases to be pending. No other operation raises a child's failure: not `spawn`, not input and output, not `sleep`.
- **Undelivered failures.** A failure is undelivered if it was not raised in the parent and the parent did not observe it with `failed(h)` or `awaitEnd(h)`. A disowned child's failure is exempt. If the program would exit with status 0 and any failure is undelivered when it ends, it exits with status 1 instead.
- **During ending.** A deferred call that raises while a task is closing replaces `Closed` (5.1), and the task fails. If its parent is marked, the failure is not raised, and it is therefore undelivered and sets the exit status: a failure at shutdown is never silent.
- **Deadlock.** If at the end of a round every live task is blocked in `await`, `awaitChange` or `awaitEnd`, no message is pending, no outside event is pending (a task sleeping or waiting for a read, a connection or a timeout is waiting for one, so it prevents `Deadlock`), and no ending step is pending (a pending `Closed`, a marked task about to become closing, or a running grace budget), `Deadlock` is raised from the blocked call of the lowest-numbered live task, which is the initial task unless it has finished. A task that has merely yielded or used up its budget is runnable, so it prevents this.
- **The end of the program.** The program ends when the initial task has ended (8.11). The initial task finishes when `start` returns or an exception leaves it, and every other task is then closed in the order of 8.11 with its deferred calls run.
- Each heap is collected independently by its own task's collector, which never pauses another task. An ended task's heap is released as a whole, and a task's queue exists as long as the task does.

### 8.7 Guarantees and costs

1. **No data races.** No memory that can change is reachable from two tasks.
2. **Determinism.** Given the same outside events, taking effect in the same rounds, every run produces the same messages, output and result, whatever the number of processors or their timing.
3. **Replay.** Recording the outside events, each with the round in which it took effect, is enough to reproduce a run exactly. The events are input, the clock, the seed, the results of file and network operations, accepts and connects, the expiry of timers and timeouts, write completions and signals.
4. **Bounded rounds.** No task can delay the end of a round by more than `N` steps plus the cost of one operation, which is itself limited by the memory limit. Because copying and zero-filling memory is priced at a bulk rate (8.4), a step is a looser measure of real time than it would be without it.
5. **Orderly ending.** Whatever ends a program, whether `start` returning, a failure or a signal, every task is closed in the order of 8.11, its deferred calls run, and the files and sockets it owns are closed, unless the task is stopped by the backstop. The ending of a tree finishes within a bounded number of rounds.

Costs:
- A message takes one round to arrive, so two tasks that interact in a tight loop advance one exchange per round. Anything that goes through a third task takes two.
- A round lasts as long as its slowest task.
- Sending and publishing take time proportional to the size of the copied graph.
- Counting steps and abstract sizes adds a small overhead to every task.
- A data structure needed by several tasks is either owned by one task, which answers requests sent to it, or published as a view (8.12). A reader is charged for the whole version it holds.
- A sender can be failed by a receiver whose queue is full.
- A task cannot hand a link to another task except by spawning it, so a service is reached through an ancestor that created both, and a service that fails is restarted together with its clients.
- No task outlives its parent.

### 8.8 Servers with several kinds of request

An inbox has one message type, so a task that accepts several kinds of request uses one record type with an integer `kind` field, named with an `enum`, together with the fields every kind needs. The compiler does not check which fields apply to which kind, and reading a field that does not apply gives its zero value. All requests share one reply type, carried in a `reply` field. See the example in section 11.

### 8.9 Compiler options

These are fixed when the program is compiled and are part of the program's identity, so a replay uses the same values (8.7). The option names and defaults below are the standard ones; an implementation may offer others.

| Option | Meaning | Default |
|---|---|---|
| `--steps` | step budget `N` per task per round (8.4) | 1,000,000 |
| `--strict-steps` | a task that exceeds its budget raises `OutOfSteps` instead of yielding (8.4) | off |
| `--memory` | live-size limit per task, in size units (8.4) | 16 Mi (16,777,216) |
| `--depth` | call-depth limit (8.5) | 10,000 |
| `--init-steps` | step limit for each initializer (4.2) | 100,000,000 |
| `--init-memory` | live-size limit for each initializer, in size units | 64 Mi |
| `--shared-memory` | size limit of the shared region, in size units | 256 Mi |
| `--grace` | rounds a closing task may run before it is stopped (8.11) | 100 |
| `--grace-time` | milliseconds a closing task may spend blocked before its blocking calls raise `Timeout` (8.11) | 5000 |
| `--socket-buffer` | bytes buffered for each socket half before a write blocks (9.7) | 65,536 |
| `--handles` | open files and sockets per task (9.3, 9.7) | 1024 |
| `--seed` | seed for `random` (9.6) | chosen from the operating system and recorded |
| `--path` | directories searched for modules, in order (4.1) | the main module's directory |

### 8.10 Child handles and links

`spawn` returns a **handle**: a `task of R` for a body with an inbox of `R`, or a `task` for a body without one. A handle belongs to the parent. It can be stored in the parent's variables and records and passed to its procedures, but it cannot be sent, passed to `spawn`, or made part of a shared or published value. The zero value is `nil`, and using it raises `NilReference`.

| Operation | Meaning |
|---|---|
| `send(h, m)` | send `m` to the child's inbox (8.2); only for a `task of R` |
| `link(h)` | return a `mailbox of R` for the child's inbox, to be passed to `spawn`; only for a `task of R` |
| `cancel(h)` | ask the child to close (8.11). It takes effect at the next boundary. On a child that has ended it still applies to the child's holders |
| `awaitEnd(h)` | block until the child has ended. It raises the child's failure if it failed and was not disowned (8.6). It is an interruption point |
| `ended(h)`, `closing(h)` | true if the child has ended, or has become closing, as of the start of the round |
| `failed(h)` | true if the child ended by failure. Observing a failure this way counts as delivery (8.6) |
| `disown(h)` | the parent will not observe the child's failure. The failure is written to the report when it happens, is never pending, and does not affect the exit status. `awaitEnd(h)` on a disowned child returns normally |

**Links and holders.**
- A task gets a link to Y from `link(h)` (the parent), from `self(queue)` (Y itself), or as a `spawn` argument.
- The **holders** of Y are the tasks to which a link to Y has been passed as a `spawn` argument, directly or through a chain of such passes. The runtime records them when `spawn` is called. Y itself is not a holder of Y, and neither is the task that called `link(h)`, which is Y's parent.
- `spawn` raises `DeadTask` if a mailbox argument names a task that is marked or has ended, judged by the state at the start of the round. This is what stops new holders from appearing once a task is closing.
- Every holder of Y is a descendant of Y's parent. A mailbox reaches a task only as a `spawn` argument, passed by a task that has it to a task that task creates, and replies, views and handles carry no link. So `cancel` reaches only tasks that the canceller created, directly or through its descendants, and never the canceller itself.
- Every holder has a greater task number than the task it holds a link to, and every child has a greater number than its parent. A task waits only for tasks with greater numbers (8.11), so the waits cannot form a cycle.

### 8.11 Ending and cancel

A task ends in a fixed order: everything it started ends first, then its outermost deferred calls run. A cancel, a failure, a return and the end of the program all use this one procedure.

**Terms.**
- A task is **cancelled** if a `cancel` covers it, as the next rule says. A task is **finished** when its body has returned or an exception has left it, and it has not yet ended. A task is **marked** if it is cancelled or finished.
- **Closure.** At every boundary, if X is cancelled then every holder of X and every descendant of X is cancelled, and if X is finished then every descendant of X is cancelled. A cancelled holder in turn cancels its own holders and descendants. A finished task does not cancel its holders, so they continue, and sending to it raises `DeadTask` once it has ended.
- A marked task becomes **closing** at the first boundary at which all its children have ended and, if it is cancelled, all its holders have ended. Everything that becomes closing at the same boundary does so together, a **wave**. Waves decide the round in which each deferred call runs, and so the order of output.
- A closing task has `Closed` pending.

**The procedure.**
1. `cancel(h)` is requested during a round and applied at step 7 of the boundary (8.3). The closure then marks the child, its holders and their descendants. A signal (below) is applied at the same step.
2. **New work stops in a marked task.** `spawn` raises `Closed`, and so does `Net::accept`. A task blocked in `Net::accept` when it becomes marked is interrupted at that boundary, and an incoming connection stays unaccepted. This holds inside deferred calls too. This `Closed` is an ordinary exception at that call, not a pending state: it can be caught, it is not repeated, and if it leaves the body the task finishes normally. Everything else a marked task does continues, so a server serves its existing clients until they are gone. `Net::connect` is allowed, so a closing task can flush to a remote service.
3. A marked task keeps running until it becomes closing.
4. A closing task has `Closed` raised at its next interruption point (below). The frames unwind and run their deferred calls, and the deferred calls of the task body run last, after everything the task started has ended.
5. When a task's body returns or an exception leaves it, the task is finished. The deferred calls of the procedures it called ran when they exited. Those of the body procedure itself run at once if the task has no child that has not ended, so a task without children ends in the round in which it returns. Otherwise they are held back, and run when the task becomes closing, after its children have ended.
6. A task ends when its deferred calls have run. A task ended by `Closed` leaving its body counts as a normal end, and is not a failure.

**Interruption points** are `sync`, `await`, `awaitChange`, `awaitEnd`, `sleep`, `sleepRounds`, and every operation that blocks on an outside event: a read, `open`, a write that waits for buffer space, `Net::accept` and `Net::connect`. The step-budget stop is not one.

**`Closed`.**
- It is raised in a closing task at an interruption point. In a marked task that is not yet closing, it is raised only by the new-work rule above.
- It is sticky: a handler that catches it does not clear it, and it is raised again at the next interruption point until the body ends.
- An interruption point inside a deferred call never raises `Closed`, so clean-up can block.
- It wins over everything that is ready. At `await`, `awaitEnd` and `awaitChange` it is raised even if a message is receivable, the child has ended or the version has changed, and the queued messages are discarded.
- A read or accept that is in progress is dropped before its data is taken, so unread input stays available.

**Backstop.** A closing task has a grace counter and a grace budget.
- The counter advances at each boundary at which the task is closing and ran in the round. A task that is idle repaying a budget debt is not counted. When the counter reaches `--grace`, the task is stopped at the boundary without running its remaining deferred calls. The runtime closes its files and listeners and ends its publishers, and the task counts as ended at that boundary.
- The budget is the time the task spends blocked while closing, for any reason, in milliseconds. When it reaches `--grace-time`, the blocking call the task is in raises `Timeout`, and so does every later blocking call it makes. The moment the budget is used up is a timer event, recorded with its round (8.7).
- A closing task that retries after `Timeout` therefore runs in every round and is stopped by the counter. A loop with no interruption point is stopped by the counter alone.

**Termination.** A marked task can start no task and receive no new holder, because `spawn` raises in it and `spawn` refuses a link to a marked task. A task waits only for tasks with greater numbers (8.10), so no cycle of waits exists. Each task becomes closing once the tasks it waits for have ended, and each closing task ends within the backstop. So the tree below a cancel ends within its depth times the backstop.

**The end of the program and signals.**
- When the initial task finishes, it is marked and the whole tree closes by the procedure above. The program ends when the initial task has ended.
- The runtime acts as the parent of the initial task. An interrupt or terminate signal is an outside event, recorded with its round. It is applied at step 7 of the boundary at which it takes effect, as a `cancel` of the initial task. When the program ends because of a signal, its status is 130 or 143. A second signal ends the program at once with the same status, without running any deferred call.

### 8.12 Views

A view is read-only state that one task publishes and any number of tasks read. `publisher of R` is the owner's unique end, and `view of R` is the end that is shared. `R` is a record type. They stand to each other as `inbox` and `mailbox` do. The zero value of each is `nil`. `Version` is a library type, `fixed(36, 0)`.

| Operation | Meaning |
|---|---|
| `newView(pub, v)` | create a pair and assign its two ends to the variables `pub` and `v` |
| `publish(pub, root)` | make the graph reachable from the reference `root` the new version of the view |
| `latest(v)` | a read-only reference to the current version, or `nil` before the first one. It is an allocation point (8.5) |
| `version(v)` | the number of the current version as a `Version`; 0 if there is none |
| `awaitChange(v, seen)` | block until the version number is above `seen`, and return it. It is an interruption point. If the owner has ended and there is nothing newer, it raises `DeadTask` |
| `ended(v)` | true if the owner has ended |

**Semantics.**
- `publish` takes a snapshot of the graph at the call, so the owner can go on changing its own copy. The version becomes visible at the next boundary (8.3, step 6). If a task publishes several times in a round, the last one wins. Version numbers count visible versions, starting at 1.
- A reference into an existing version or into the shared region that the graph reaches is copied by identity and keeps that version alive.
- A version cannot be changed: a write through a reference into one raises `ReadOnly`.
- A published graph may contain plain data. It may not contain a `mailbox`, `reply`, `view`, `inbox`, `publisher`, `task`, `File` or `Listener`.
- A `view` is passed to other tasks in a message or as a `spawn` argument. The `publisher` moves only as a `spawn` argument.
- If the owner ends, the last version stays readable and `ended` is true. A reader woken by the last publish sees that version, and its next `awaitChange` raises `DeadTask`.
- `Closed` wins over a changed version at `awaitChange` (8.11). A reader that missed versions can tell how many: `new - seen - 1`.
- `newView`, `publish` and `awaitChange` raise `BadContext` in a `share` initializer.

**Accounting.** A version lives while any task can reach it, by exact reachability as in 8.5, not by collector timing. It is charged as follows:
a. **The owner** pays for the current version of each of its views, from the call to `publish` until a newer version becomes visible. `publish` is an allocation: it raises `OutOfMemory` if the owner's live size together with the snapshot would exceed the limit. Two publishes in one round are both charged until the boundary.
b. **A reader** is charged the full size of everything it can reach in a version, counted once however many references point to it, as if it held a copy. `latest` is an allocation point.
c. **A message** that carries a reference into a version counts, in the receiver's queue, the units reachable from that reference.
d. **After the owner has ended**, a `view` costs 1 plus the units of its last version for every task that can reach it. The runtime keeps, for each view, the set of tasks that were given it, by a message or a `spawn` argument, and the owner. When the owner ends, it adds the size of the last version to those tasks' units allocated since (8.5) and checks the limits at that boundary.

Every byte of a live version is charged to a living task: the owner while the version is current, a reader or a receiver that can reach it, or the holders of the view once the owner has ended. A version that only another version reaches is counted in that version's reachable size. So the memory of all versions is bounded by the sum of the task limits, without a scan of the heaps.

**Price.** `publish` is priced as a copy of the graph (8.4). The semantics have no dirty tracking, and an implementation may share unchanged parts between versions, so the real cost can be far lower than the price.

The owner of a view needs room for its own copy and for the snapshot. A ledger of 100,000 records of 10 units each costs 100,000 + 62,500 steps per publish, and while the version is current the owner's live size is the ledger twice.

## 9. Standard library

The standard library is the module `std`, imported into every module automatically (4.1). Its names can be used unqualified, and a module can hide them with its own declarations.

| Name | Meaning |
|---|---|
| `len(a)` | length of any array |
| `ord(c)`, `chr(n)` | character code conversion |
| `String` | `ref array of char` |
| `stringcmp(a, b)` | -1, 0 or 1 by lexicographic order of char codes |
| `concat(a, b)` | new string containing `a` then `b` |
| `stringEqual(a, b)` | true if the strings have the same length and characters |
| `hashInt(n)`, `hashString(s)` | a non-negative `int` computed from the value alone, the same on every run and in every implementation: 64-bit FNV-1a over the 8 bytes of `n` (most significant first), or over the UTF-8 bytes of `s`, with the top bit cleared |
| `assert(c)` | raises `AssertFailed` if the `bool` `c` is false; the uncaught-exception report shows the source text of `c` |
| `spawn(body, args...)` | start a task and return its handle, a `task of R` or a `task` (see 8.1, 8.10) |
| `send(mb, m)`, `receive(q, m)`, `await(q, m)`, `sync()`, `self(q)` | task communication (see 8.2, 8.3) |
| `replyTo(q)`, `replyTo(q, n)`, `answer(r, m)` | replies that take one answer, or up to `n` (see 8.2) |
| `link(h)`, `cancel(h)`, `awaitEnd(h)`, `ended(h)`, `failed(h)`, `closing(h)`, `disown(h)` | operations on a child handle (see 8.10) |
| `newView(pub, v)`, `publish(pub, root)`, `latest(v)`, `version(v)`, `awaitChange(v, seen)`, `ended(v)` | published state (see 8.12) |
| `Version` | `fixed(36, 0)`, the type of view version numbers |
| `Round` | `fixed(36, 0)`, the type of round numbers |

`len`, `fixedStr`, `strFixed`, `abs`, `min`, `max`, `scaleOf`, `power`, `floor`, `ceiling`, `truncate`, `sign`, `units`, `fromUnits`, `divide`, `rescale` and the task procedures are polymorphic built-ins checked by the compiler. Everything else is ordinary code in the library. Entry point, I/O, strings, numbers, time and randomness follow. Generic modules (4.4) let collections such as lists and maps be written once for any element type; none is defined in this version (section 12). The module `Net` (9.7) is provided by the implementation.

### 9.1 Entry point

- A program declares a procedure named `start`, in one of four forms: `start()`, `start(): int`, `start(queue: inbox of R)` or `start(queue: inbox of R): int`. It is the body of the initial task (8.1). `R` is any record type, as for any task body.
- The `inbox` form gives the initial task a queue, so that tasks it spawns can reply to it through `self(queue)`. Without that parameter the initial task cannot receive messages.
- Before round 1, the shared values are built (4.2); `start` runs in round 1.
- The program ends when `start` has ended, which is after every task it started has been closed (8.11). A `proc` form exits with status 0, and an `int` form exits with the value it returns, which must be in 0 to 255, otherwise the `return` raises `Overflow`. An exception that leaves `start` exits with status 1 after the report in 7.5.
- `args()` returns the command-line arguments as a `ref array of String`, not including the program name. Each task gets its own copy.

### 9.2 Text and standard streams

| Name | Meaning |
|---|---|
| `write(s)`, `writeLine(s)` | write the `String` `s` to standard output, `writeLine` followed by a newline |
| `writeErr(s)`, `writeErrLine(s)` | the same for standard error |
| `readLine()` | the next line of standard input as a `String`, without its terminator (`\n` or `\r\n`), or `nil` at end of input |
| `intStr(n)`, `fixedStr(x)`, `charStr(c)` | the decimal text of an `int`, of a `fixed` value with all its scale digits (`-1.50`, `0.00`, `7`), and a one-character string. A leading `-` marks a negative number |
| `strInt(s)` | the `int` written in `s`, or `FormatError`; `Overflow` if it is out of range |
| `strFixed(s, k)` | the number written in `s` as an exact `fixed` of scale `k`, a constant expression; `FormatError` if `s` is not a number |

- **Number text** is ASCII digits with an optional leading `-`, and for `fixed` a `.` as the decimal point, with at least one digit on each side. There is no `+`, no spaces, no exponent and no locale; empty text is a `FormatError`. Leading zeros are accepted.
- **`strFixed`** pads with zeros when `s` has fewer than `k` fractional digits (`"1.5"` with `k = 2` is `1v50`). When `s` has more, the extra digits are a rounding step (7.4): they must all be zeros, or `LostPrecision` is raised, unless the statement says `rounded`. Any number of integer digits is accepted, and `Overflow` is raised at the assignment.
- Text is UTF-8. `char` is a code point, so `len` of a string is a count of code points.
- There is no formatting language. A program builds its output with `intStr`, `fixedStr`, `charStr` and `concat`.
- Output is buffered per task and written at the end of the round, in task order (8.3). Standard output and standard error are separate streams, each ordered that way.
- `readLine()` is an outside event. If a line is available at the start of a round, it returns in that round, otherwise the task blocks, as in `await`, until the first round in which one is. A blocked reader prevents `Deadlock` (8.6) while standard input is still open.
- If several tasks are blocked on standard input when a line arrives, the lowest-numbered one receives it and the others stay blocked.
- A task that must wait for either a line or a message runs the reader in a separate task, which sends each line to the waiting task as a message.
- Costs: `write` is charged one step per size unit of its string, and `readLine` the size units of its result (8.4).

### 9.3 Files and bytes

| Name | Meaning |
|---|---|
| `File` | predefined opaque reference type; zero value `nil` |
| `open(path, mode)` | open the file named by the `String` `path`; `mode` is `ModeRead`, `ModeWrite` (truncate or create) or `ModeAppend` |
| `close(f)` | close the file, flushing buffered output |
| `stdin()`, `stdout()`, `stderr()` | this task's handles for the standard streams |
| `readLine(f)`, `write(f, s)`, `writeLine(f, s)` | the text operations above, on any `File` |
| `exists(path)` | true if a file or directory of that name exists |
| `delete(path)` | delete a file or an empty directory; `FileNotFound` if absent |
| `rename(from, to)` | rename a file or directory, replacing an existing file named `to` |
| `mkdir(path)` | create one directory whose parent exists; `IOError` if it exists already |
| `list(path)` | the entry names of a directory as a `ref array of String`, sorted by code point, without `.` and `..`; `FileNotFound` if absent |
| `readBytes(f, n)` | up to `n` bytes as a `ref array of int` with each element 0 to 255, or `nil` at end of file |
| `readLine(f, timeout)`, `readBytes(f, n, timeout)` | the same reads with a time limit in milliseconds; `Timeout` is raised if nothing arrives in time, and a negative limit raises `NegativeSize` |
| `writeBytes(f, a)` | write the elements of `a` as bytes; an element outside 0 to 255 raises `Overflow` |

- `write(s)` means `write(stdout(), s)` and `readLine()` means `readLine(stdin())`.
- A `File` belongs to one task. It can pass to another only as an argument of `spawn`, which moves it (8.1): the sender's variable becomes `nil`. The compiler rejects a message type that contains a `File`, and a record that contains one as a `spawn` argument. A task's files are closed, in the order opened, when the task ends. The files a task owns include the ones it was given.
- Text operations and byte operations on the same file may be mixed. Text operations decode and encode UTF-8 and byte operations do not.
- Every read and `open` is an outside event: it blocks the task and completes in the first round in which its result is available, and the result is recorded for replay (8.7). Writes are buffered and are written in task order at the end of the round. A write that fails then is reported by raising `IOError` from the same task's next operation on that file, or from `close`.
- `exists`, `delete`, `rename`, `mkdir` and `list` work on paths, not on open files, and like reads are outside events that block the task until they complete. They raise `PermissionDenied` or `IOError` for failures not named above.
- Using a file after `close` raises `IOError`, and using `nil` raises `NilReference`.
- A byte costs one size unit and one step.

### 9.4 Strings

These are ordinary library code over `String`. Every result is a new string.

| Name | Meaning |
|---|---|
| `substring(s, start, n)` | the characters from index `start`, at most `n` of them. `start` outside 0 to `len(s)` raises `IndexRange`, and a negative `n` raises `NegativeSize` |
| `find(s, t, from)` | the lowest index at or after `from` where `t` occurs in `s`, or -1. `from` outside 0 to `len(s)` raises `IndexRange`; an empty `t` is found at `from` |
| `startsWith(s, t)`, `endsWith(s, t)` | `bool` |
| `join(parts, sep)` | the strings of the `ref array of String` `parts` with `sep` between them |
| `split(s, sep)` | a `ref array of String` of the pieces of `s` between occurrences of `sep`, always with at least one element; adjacent separators give empty pieces. An empty `sep` raises `FormatError` |
| `repeat(s, n)` | `n` copies of `s`, joined; a negative `n` raises `NegativeSize` |
| `padLeft(s, width, c)`, `padRight(s, width, c)` | `s` extended with the char `c` on the left or right to `width` chars; a copy of `s` if it is already long enough |
| `trim(s)` | `s` without leading and trailing space, tab, line feed and carriage return |

A `nil` string passed to any of them raises `NilReference`.

### 9.5 Numbers

| Name | Meaning |
|---|---|
| `abs(x)`, `min(a, b)`, `max(a, b)` | on `int` or `fixed`. For `fixed` the result has the larger scale of its operands. `abs` of the smallest `int` raises `Overflow` |
| `gcd(a, b)` | the non-negative greatest common divisor of two `int`s, with `gcd(0, 0) = 0` |
| `power(b, e)` | `b` to the power `e`, which must not be negative (`NegativeSize`). For an `int` base the result is an `int`, with `Overflow` as usual. For a `fixed` base, `e` must be a constant expression; the result is exact, its scale is the scale of `b` times `e`, and `power(b, 0)` is the `int` 1. For example, `power(1v10, 2)` is `1v2100` |
| `floor(x)`, `ceiling(x)`, `truncate(x)` | the `int` nearest to `x` rounding toward minus infinity, toward plus infinity, or toward zero; `x` is an `int` or a `fixed`; `Overflow` if the result does not fit |
| `sign(x)` | -1, 0 or 1, by the sign of the `int` or `fixed` `x` |
| `units(x)` | the `int` count of the smallest units of the `fixed` `x`, that is `x` times 10 to the power `scaleOf(x)`: `units(12v50)` is 1250; `Overflow` if it does not fit |
| `fromUnits(n, k)` | the exact `fixed` of scale `k` (a constant) whose value is `n` divided by 10 to the power `k`: `fromUnits(1250, 2)` is `12v50` |
| `divide(a, b, k)`, `divide(a, b, k, mode)` | the quotient of `a` and `b` as a `fixed` of scale `k`, rounded once (7.4) |
| `rescale(x, k)`, `rescale(x, k, mode)` | `x` as a `fixed` of scale `k` (7.4) |
| `RoundHalfAway`, `RoundHalfEven`, `RoundDown`, `RoundUp`, `RoundFloor`, `RoundCeiling` | the rounding modes (7.4) |
| `scaleOf(x)` | the scale of the `fixed` expression `x`, as a constant `int` |
| `band(a, b)`, `bor(a, b)`, `bxor(a, b)`, `bnot(a)` | bitwise operations on the 64-bit two's-complement form of `int`s. They never overflow |
| `wrapAdd(a, b)`, `wrapSub(a, b)`, `wrapMul(a, b)` | the low 64 bits of the exact sum, difference or product, as a two's-complement `int`. They never raise `Overflow` |
| `rotl(a, n)`, `rotr(a, n)` | rotate the 64 bits of `a` left or right by `n` places. `n` must not be negative (`NegativeSize`); the count is taken modulo 64 |
| `shl(a, n)`, `shr(a, n)` | shift left, and shift right with sign extension. `n` must not be negative (`NegativeSize`); bits shifted out are lost, and a shift by 64 or more gives 0 (`shl`), or 0 or -1 by the sign (`shr`) |

### 9.6 Time and randomness

| Name | Meaning |
|---|---|
| `now()` | the time as an `int` of milliseconds since 1970-01-01 00:00 UTC, sampled once at the start of each round, so every task sees the same value in a round |
| `sleep(ms)` | blocks the task until the first round that starts at or after `now() + ms`. A negative `ms` raises `NegativeSize`; `sleep(0)` is the same as `sync()` |
| `round()` | the number of the current round as a `Round`, the same for every task in a round. The first round is 1, and in a `share` initializer the value is 0. It is derived from the run, so it is not recorded |
| `sleepRounds(n)` | blocks the task until the start of round `round() + n`. A negative `n` raises `NegativeSize`; `sleepRounds(0)` is the same as `sync()`. It is an interruption point and raises `BadContext` in an initializer |
| `random(n)` | an `int` `r` with `0 <= r < n`; `n` less than 1 raises `NegativeSize` |

- The clock value of each round and the seed are outside events and are recorded for replay (8.7). A round does not wait for time to pass unless every task is blocked and a sleeper is the next thing due; then the program waits until the earliest wake-up time. A task in `sleepRounds` does not wait for time: when every task is blocked or sleeping by rounds, the rounds go on at once. `Round` is `fixed(36, 0)`, as `Version` is. Together with `--strict-steps` (8.4), `round()` is a logical clock, and `sleepRounds` gives ticks that do not depend on the wall clock.
- Each task has its own random sequence, determined by the run seed (8.9) and the task number. The same seed and the same task numbering give the same numbers. The algorithm is fixed for a given implementation version.
- If the operating system asks the program to stop (an interrupt or terminate signal), the initial task is cancelled and the whole program closes in the order of 8.11, with status 130 or 143. A second signal ends the program at once. Programs cannot handle signals other than by being closed.

### 9.7 Sockets: the `Net` module

`Net` is a module provided by the implementation (4.1). A program uses it with `import Net;`, and its names are written `Net::name`.

| Name | Meaning |
|---|---|
| `Net::Listener` | predefined opaque type, a listening socket; zero value `nil` |
| `Net::listen(port)` | start listening on the `int` port and return a `Net::Listener`; `Net::AddressInUse` if the port is taken |
| `Net::accept(l, input, output)`, `Net::accept(l, input, output, timeout)` | block until a connection arrives, then assign its two halves, as `File`s, to the variables `input` and `output`. This is new work (8.11) |
| `Net::connect(host, port, input, output)`, `Net::connect(host, port, input, output, timeout)` | open a connection to the `String` `host` and the `int` `port` and assign its two halves |
| `Net::close(l)` | stop listening |

Exceptions: `Net::ConnectionRefused`, `Net::ConnectionReset`, `Net::AddressInUse`, `Net::Unreachable`, `IOError` for other failures, and `Timeout` when a time limit given in milliseconds expires. A negative limit raises `NegativeSize`.

- A connection is two `File` halves, one for input and one for output, and all the file operations of 9.3 apply to them. A half belongs to one task and moves to another only as an argument of `spawn`, like any `File`. A `Net::Listener` moves the same way. The runtime closes what a task owns when the task ends.
- **Reads** block until data or the end of input arrives. The data and the round in which it arrived are recorded for replay (8.7). `readLine` and `readBytes` return `nil` at the end of input.
- **Writes** go into a buffer of `--socket-buffer` bytes for each half and are sent at the round boundary. A write that finds the buffer full blocks, so a slow peer stalls only the task that writes to it. The round in which a block ends is a recorded outside event. A write to a peer that has reset the connection raises `Net::ConnectionReset`.
- **Closing** the output half tells the peer that the input has ended.
- **Without a `select`**, a task cannot wait for a socket and for its inbox at once. A protocol with independent flows in the two directions uses two tasks for a connection, one owning each half.
- **Replay.** The network is not touched when a run is replayed: reads come from the log, writes are discarded, and name resolution, accepts, connects and timeouts are taken from the log.
- Every operation of `Net` raises `BadContext` in an initializer (4.2). The number of open files and sockets of a task is limited by `--handles`, and exceeding it raises `IOError`.
- Costs: each operation is 1 step, and a byte read or written is one step and one size unit.
- Not in this version: TLS and datagrams. A TLS wrapper can later turn a pair of halves into another pair.

## 10. Grammar

```ebnf
program     = { genparam } { importdecl }
              { ( [ "also" ] [ "private" ]
                  ( constdecl | enumdecl | typedecl | exdecl | sharedecl
                  | procdecl ) )
              | instdecl } .
genparam    = "generic" ( "type" ident
                        | "const" ident ":" type
                        | "proc" ident "(" [ params ] ")" [ ":" type ] ) ";" .
importdecl  = "import" importitem { "," importitem } ";" .
importitem  = ident [ "is" ident "(" garg { "," garg } ")" ] .
instdecl    = "import" ident "is" ident "(" garg { "," garg } ")" ";" .
garg        = type | expr .
constdecl   = "const" ident "is" expr ";" .
enumdecl    = "enum" ident { "," ident } ";" .
typedecl    = "type" ident "is" type ";" .
exdecl      = "exception" ident ";" .
sharedecl   = "share" ident ":" type "is" expr ";" .
vardecl     = "var" ident ":" type ";" .
procdecl    = "proc" ident "(" [ params ] ")" [ ":" type ] block ";" .
params      = param { "," param } .
param       = ident ":" ( type | proctype ) .
proctype    = "proc" "(" [ type { "," type } ] ")" [ ":" type ] .
block       = { vardecl } "begin" stmts "end" .
stmts       = { statement ";" } .

type        = qident
            | "int" | "bool" | "char"
            | "fixed" "(" expr "," expr ")"
            | "array" "[" expr "]" "of" type
            | "record" { [ "private" ] ident ":" type ";" } "end"
            | "ref" ( qident | "array" "of" type )
            | "inbox" "of" qident
            | "mailbox" "of" qident
            | "reply" "of" qident
            | "view" "of" qident
            | "publisher" "of" qident
            | "task" [ "of" qident ] .

statement   = designator ":=" [ "rounded" ] expr
            | [ "void" ] qident "(" [ args ] ")"
            | "if" expr "then" stmts { "elsif" expr "then" stmts }
              [ "else" stmts ] "end"
            | "while" expr "do" stmts "end"
            | "return" [ expr ]
            | "exit" | "next"
            | "defer" [ "void" ] qident "(" [ args ] ")"
            | "raise" [ qident ]
            | "try" stmts handler { handler } "end" .
handler     = "catch" [ qident { "," qident } ] "do" stmts .

qident      = ident [ "::" ident ] .
designator  = qident { "[" expr "]" | "." ident } .
args        = expr { "," expr } .

expr        = conj { "or" conj } .
conj        = neg { "and" neg } .
neg         = "not" neg | rel .
rel         = sum [ ( "=" | "<" | "<=" | ">" | ">=" | "~" ) sum ] .
sum         = term { ( "+" | "-" ) term } .
term        = unary { ( "*" | "/" | "div" | "mod" ) unary } .
unary       = "_" unary | factor .
factor      = number | fixedlit | charlit | stringlit
            | "true" | "false" | "nil"
            | designator
            | qident "(" [ args ] ")" { "[" expr "]" | "." ident }
            | "new" "(" type ")"
            | "new" "(" "array" "of" type "," expr ")"
            | "(" expr ")" .

ident       = letter { letter | digit } .
letter      = ? an ASCII letter ? .
digit       = ? an ASCII digit ? .
number      = digit { digit } | "0x" hexdigit { hexdigit } .
hexdigit    = digit | ? one of A to F, a to f ? .
charlit     = ? a single quote, one character or escape, a single quote ? .
stringlit   = ? a double quote, characters and escapes, a double quote ? .
fixedlit    = number "v" number .
comment     = "#" { any character except end of line } .
```

## 11. Examples

### Binary search in a balanced tree

```
type Node is record
  key: int;
  left: ref Node;
  right: ref Node;
end;

proc find(root: ref Node, k: int): ref Node
  var cur: ref Node;
begin
  cur := root;
  while not (cur ~ nil) do
    if k = cur.key then
      return cur;
    elsif k < cur.key then
      cur := cur.left;
    else
      cur := cur.right;
    end;
  end;
  return nil;
end;

proc build(keys: ref array of int, lo: int, hi: int): ref Node
  var n: ref Node;
  var mid: int;
begin
  if hi < lo then
    return nil;
  end;
  mid := (lo + hi) div 2;
  n := new(Node);
  n.key := keys[mid];
  n.left := build(keys, lo, mid - 1);
  n.right := build(keys, mid + 1, hi);
  return n;
end;
```

`build` takes sorted keys and returns a perfectly balanced tree; `find` takes about log₂ n steps.

### Strings

```
type String is ref array of char;

proc concat(a: String, b: String): String
  var r: String;
  var i: int;
begin
  r := new(array of char, len(a) + len(b));
  i := 0;
  while i < len(a) do
    r[i] := a[i];
    i := i + 1;
  end;
  i := 0;
  while i < len(b) do
    r[len(a) + i] := b[i];
    i := i + 1;
  end;
  return r;
end;

proc stringcmp(a: String, b: String): int
  var i: int;
begin
  i := 0;
  while (i < len(a)) and (i < len(b)) do
    if a[i] < b[i] then
      return _1;
    elsif b[i] < a[i] then
      return 1;
    end;
    i := i + 1;
  end;
  if len(a) < len(b) then
    return _1;
  elsif len(b) < len(a) then
    return 1;
  end;
  return 0;
end;
```

### Procedure parameters

```
proc ascending(x: int, y: int): bool
begin
  return x < y;
end;

proc insertionSort(a: ref array of int, less: proc(int, int): bool)
  var i: int;
  var j: int;
  var x: int;
begin
  i := 1;
  while i < len(a) do
    x := a[i];
    j := i - 1;
    while (j >= 0) and less(x, a[j]) do
      a[j + 1] := a[j];
      j := j - 1;
    end;
    a[j + 1] := x;
    i := i + 1;
  end;
end;
```

Calling `insertionSort(data, ascending);` sorts `data` in place. Short-circuit `and` is what stops `a[_1]` from being evaluated.

### Fixed-point and exceptions

```
exception BadPrice;

proc addTax(price: fixed(7,2), tax: fixed(7,2))
begin
  tax := rounded price * 0v19;   # 3.7981 rounds to 3.80
  if tax < 0v00 then
    raise BadPrice;
  end;
end;

proc safeAdd(): fixed(7,2)
  var price: fixed(7,2);
  var tax: fixed(7,2);
begin
  price := 19v99;
  try
    addTax(price, tax);
  catch BadPrice do
    tax := 0v00;
  catch do
    tax := 0v00;
  end;
  return tax;
end;

proc split(total: fixed(7,2), n: fixed(3,0)): fixed(7,2)
  var part: fixed(7,2);
begin
  part := rounded total / n;
  return part;
end;

proc parsePrice(text: String): fixed(7,2)
  var price: fixed(7,2);
begin
  price := strFixed(text, 2);
  return price;
end;
```

Without `rounded`, the assignment to `tax` raises `LostPrecision`, because the digits 81 would be lost. In `split`, the quotient has scale 2 (scale 2 plus scale 0), and `rounded` rounds it once; without `rounded`, `split(1v00, 3)` raises `LostPrecision`. `parsePrice("1.5")` returns `1v50`, `parsePrice("1.999")` raises `LostPrecision`, and `parsePrice("abc")` raises `FormatError`.

### Tasks: a store and a client

```
const Size is 100;
enum Get, Put;

type Reply is record
  found: bool;
  value: int;
end;

type Request is record
  kind: int;
  key: int;
  value: int;
  back: reply of Reply;
end;

proc store(queue: inbox of Request)
  var table: array [Size] of int;
  var present: array [Size] of bool;
  var req: ref Request;
  var rep: ref Reply;
begin
  while true do
    await(queue, req);
    rep := new(Reply);
    if req.kind = Put then
      table[req.key] := req.value;
      present[req.key] := true;
    elsif req.kind = Get then
      rep.found := present[req.key];
      rep.value := table[req.key];
    end;
    answer(req.back, rep);
  end;
end;

proc client(queue: inbox of Reply, server: mailbox of Request,
            parent: mailbox of Reply)
  var req: ref Request;
  var rep: ref Reply;
begin
  req := new(Request);
  req.kind := Put;
  req.key := 7;
  req.value := 42;
  req.back := replyTo(queue);
  send(server, req);
  await(queue, rep);
  req := new(Request);
  req.kind := Get;
  req.key := 7;
  req.back := replyTo(queue);
  send(server, req);
  await(queue, rep);
  send(parent, rep);
end;

proc start(queue: inbox of Reply): int
  var s: task of Request;
  var rep: ref Reply;
begin
  s := spawn(store);
  void spawn(client, link(s), self(queue));
  await(queue, rep);
  writeLine(concat("value ", intStr(rep.value)));
  return 0;
end;
```

Each request takes two rounds: one for it to arrive and one for the answer. `start` prints `value 42` and exits with status 0. `spawn(store)` returns a `task of Request`, and `link(s)` gives the client a link to it, which makes the client a holder of `store` (8.10). When `start` returns, it is finished, and `store` and `client` are cancelled. `client` has already ended. `store`, blocked in `await`, becomes closing once its holder has ended, gets `Closed` there and ends, and then `start` ends and the program exits. After the second `await`, `rep.found` is true and `rep.value` is 42. An out-of-range `key` raises `IndexRange` inside `store`, which ends that task as a failed one: the failure is written to standard error, and is raised in `start` at its next `sync`, `await` or `awaitChange`, which here is the `await` it is waiting in.

### Tasks: published state

```
type Counter is record
  value: int;
end;

type Done is record
  seen: int;
end;

proc ticker(pub: publisher of Counter)
  var c: ref Counter;
begin
  c := new(Counter);
  while c.value < 3 do
    c.value := c.value + 1;
    publish(pub, c);
    sync();
  end;
end;

proc watcher(feed: view of Counter, parent: mailbox of Done)
  var seen: Version;
  var c: ref Counter;
  var d: ref Done;
begin
  seen := 0;
  try
    while true do
      seen := awaitChange(feed, seen);
    end;
  catch DeadTask do
  end;
  c := latest(feed);
  d := new(Done);
  d.seen := c.value;
  send(parent, d);
end;

proc start(queue: inbox of Done): int
  var pub: publisher of Counter;
  var feed: view of Counter;
  var d: ref Done;
begin
  newView(pub, feed);
  void spawn(watcher, feed, self(queue));
  void spawn(ticker, pub);
  await(queue, d);
  writeLine(concat("last ", intStr(d.seen)));
  return 0;
end;
```

`newView` makes the two ends. The view `feed` is copied by identity into `watcher`, and the publisher `pub` is moved into `ticker`, so `pub` is `nil` in `start` after the call. `ticker` publishes in rounds 2, 3 and 4, each version becoming visible at the end of its round, and ends in round 5. `watcher` wakes in rounds 3, 4 and 5 and ends its wait when `awaitChange` raises `DeadTask`, which it does in round 6 because the owner has ended and nothing newer exists. It then reads the last version, which is 3, and tells `start`, which prints `last 3` and exits with status 0.

### Sockets: an echo server

```
import Net;

proc echo(input: File, output: File)
  var line: String;
begin
  defer close(input);
  defer close(output);
  try
    line := readLine(input);
    while not (line ~ nil) do
      writeLine(output, line);
      line := readLine(input);
    end;
  catch Net::ConnectionReset, IOError do
  end;
end;

proc start(): int
  var listener: Net::Listener;
  var input: File;
  var output: File;
begin
  listener := Net::listen(8080);
  defer Net::close(listener);
  while true do
    Net::accept(listener, input, output);
    disown(spawn(echo, input, output));
  end;
  return 0;
end;
```

Each connection gets a task of its own, and `spawn` moves the two halves into it, leaving `input` and `output` `nil` for the next `accept`. `disown` says that `start` will not look at the failure of an `echo` task: it is written to the report, and does not affect the exit status. A client that drops the connection is caught inside `echo` and is not a failure. If a signal arrives, `start` is cancelled: its blocked `accept` raises `Closed`, the loop is left, the `echo` tasks are cancelled and get `Closed` in their reads, and `start` closes the listener last, in its deferred call, before the program exits with status 130 or 143.

### Modules

Module `IntList`:

```
exception Empty;

type Cell is record
  value: int;
  link: ref Cell;
end;

type List is record
  private head: ref Cell;
  private size: int;
end;

private proc drop(l: List)
begin
  l.head := l.head.link;
  l.size := l.size - 1;
end;

proc push(l: List, x: int)
  var c: ref Cell;
begin
  c := new(Cell);
  c.value := x;
  c.link := l.head;
  l.head := c;
  l.size := l.size + 1;
end;

proc pop(l: List): int
  var x: int;
begin
  if l.head ~ nil then
    raise Empty;
  end;
  x := l.head.value;
  drop(l);
  return x;
end;
```

Main module `App`:

```
import IntList;

proc start(): int
  var l: IntList::List;
  var x: int;
begin
  IntList::push(l, 1);
  IntList::push(l, 2);
  x := IntList::pop(l);
  try
    void IntList::pop(l);
    void IntList::pop(l);
  catch IntList::Empty do
    writeLine("empty");
  end;
  return x;
end;
```

The program prints `empty` and exits with status 2. `IntList::drop` is private, so `App` could not call it.

### Shared data

Module `Tables`:

```
private proc squares(n: int): ref array of int
  var a: ref array of int;
  var i: int;
begin
  a := new(array of int, n);
  i := 0;
  while i < n do
    a[i] := i * i;
    i := i + 1;
  end;
  return a;
end;

share Squares: ref array of int is squares(1000);

proc square(i: int): int
begin
  return Squares[i];
end;
```

Main module `Demo`:

```
import Tables;

proc start(): int
begin
  try
    Tables::Squares[3] := 0;
  catch ReadOnly do
    writeLine("read-only");
  end;
  return Tables::square(12);
end;
```

`squares(1000)` runs once, before round 1. Every task reads `Squares` without copying it. The program prints `read-only` and exits with status 144.

### Loops, escapes and library calls

```
proc countWords(text: String): int
  var parts: ref array of String;
  var i: int;
  var n: int;
begin
  parts := split(text, " ");
  i := 0;
  n := 0;
  while i < len(parts) do
    if len(parts[i]) = 0 then
      i := i + 1;
      next;
    end;
    n := n + 1;
    i := i + 1;
    if n = 100 then
      exit;
    end;
  end;
  return n;
end;

proc start(): int
begin
  writeLine("tab:\t| quote:\" | smile: \u{1F600}");
  assert(countWords("a  b c") = 3);
  return band(0xFF, 0x0F);
end;
```

`split("a  b c", " ")` gives `"a"`, `""`, `"b"`, `"c"`; `next` skips the empty piece. The program exits with status 15.

### Explicit rounding and units

```
proc start(): int
  var a: fixed(7,3);
  var b: fixed(7,2);
  var c: fixed(7,4);
  var n: int;
begin
  a := divide(1v0, 8v0, 3);
  b := rescale(2v675, 2, RoundHalfEven);
  c := divide(1v00, 3v00, 4, RoundHalfAway);
  n := floor(_2v5);
  n := n + units(12v50) + sign(c);
  return n mod 256;
end;
```

`a` is `0v125`, `b` is `2v68`, `c` is `0v3333`, and `n` ends as -3 + 1250 + 1 = 1248, so the exit status is 224.

### Deferred calls

```
proc show(path: String)
  var f: File;
  var line: String;
begin
  f := open(path, ModeRead);
  defer close(f);
  line := readLine(f);
  while not line ~ nil do
    writeLine(line);
    line := readLine(f);
  end;
end;
```

`close(f)` runs when `show` exits, whether it reaches its end or an exception leaves it.

### Hiding a library name

```
proc max(a: String, b: String): String
begin
  if stringcmp(a, b) >= 0 then
    return a;
  end;
  return b;
end;

proc start(): int
begin
  writeLine(max("apple", "pear"));
  return std::max(3, 9);
end;
```

Inside this module `max` is the procedure above. `std::max` is still the library one. The program prints `pear` and exits with status 9.

### Named number types

```
type Dollars is fixed(12,2);
type Euros is fixed(12,2);

proc start(): int
  var price: Dollars;
  var tax: Dollars;
  var total: Dollars;
  var fare: Euros;
begin
  price := 19v99;
  tax := rounded price * 0v19;
  total := price + tax;
  fare := 5v00;
  return truncate(total);
end;
```

`tax` is 3.80 and `total` is 23.79, so the exit status is 23. `price + fare` and `total := fare` are compile errors.

### Recursive groups

```
type Tree is record
  label: int;
  kids: ref Forest;
end;

also type Forest is record
  trees: ref array of ref Tree;
end;

proc countNodes(t: ref Tree): int
  var total: int;
  var i: int;
begin
  if t ~ nil then
    return 0;
  end;
  total := 1;
  if not t.kids ~ nil then
    i := 0;
    while i < len(t.kids.trees) do
      total := total + countNodes(t.kids.trees[i]);
      i := i + 1;
    end;
  end;
  return total;
end;

proc isEven(n: int): bool
begin
  if n = 0 then
    return true;
  end;
  return isOdd(n - 1);
end;

also proc isOdd(n: int): bool
begin
  if n = 0 then
    return false;
  end;
  return isEven(n - 1);
end;

proc start(): int
  var root: ref Tree;
  var forest: ref Forest;
begin
  root := new(Tree);
  forest := new(Forest);
  forest.trees := new(array of ref Tree, 2);
  forest.trees[0] := new(Tree);
  forest.trees[1] := new(Tree);
  root.kids := forest;
  if isEven(countNodes(root)) then
    return 0;
  end;
  return 1;
end;
```

`countNodes(root)` is 3, so the exit status is 1.

### A generic module

Module `Stack`:

```
generic type T;

exception Empty;

type Cell is record
  value: T;
  link: ref Cell;
end;

type Stack is record
  top: ref Cell;
  private size: int;
end;

proc push(s: Stack, x: T)
  var c: ref Cell;
begin
  c := new(Cell);
  c.value := x;
  c.link := s.top;
  s.top := c;
  s.size := s.size + 1;
end;

proc pop(s: Stack): T
  var x: T;
begin
  if s.top ~ nil then
    raise Empty;
  end;
  x := s.top.value;
  s.top := s.top.link;
  s.size := s.size - 1;
  return x;
end;

proc count(s: Stack): int
begin
  return s.size;
end;
```

Module `Lookup`, with a procedure parameter:

```
generic type K;
generic type V;
generic proc equal(a: K, b: K): bool;

exception Missing;

type Entry is record
  key: K;
  value: V;
end;

type Table is record
  entries: ref array of Entry;
  private used: int;
end;

proc get(t: Table, key: K): V
  var i: int;
begin
  i := 0;
  while i < t.used do
    if equal(t.entries[i].key, key) then
      return t.entries[i].value;
    end;
    i := i + 1;
  end;
  raise Missing;
end;

proc put(t: Table, key: K, value: V)
  var bigger: ref array of Entry;
  var i: int;
begin
  i := 0;
  while i < t.used do
    if equal(t.entries[i].key, key) then
      t.entries[i].value := value;
      return;
    end;
    i := i + 1;
  end;
  if t.entries ~ nil then
    t.entries := new(array of Entry, 4);
  elsif t.used = len(t.entries) then
    bigger := new(array of Entry, t.used * 2);
    i := 0;
    while i < t.used do
      bigger[i] := t.entries[i];
      i := i + 1;
    end;
    t.entries := bigger;
  end;
  t.entries[t.used].key := key;
  t.entries[t.used].value := value;
  t.used := t.used + 1;
end;
```

Main module `Demo`:

```
import Numbers is Stack(int), Ages is Lookup(String, int, stringEqual);

proc start(): int
  var s: Numbers::Stack;
  var ages: Ages::Table;
begin
  Numbers::push(s, 4);
  Numbers::push(s, 38);
  Ages::put(ages, "ann", 31);
  Ages::put(ages, "ann", 32);
  return Numbers::pop(s) + Numbers::pop(s) + Ages::get(ages, "ann") - 32;
end;
```

The program exits with status 42. `Numbers::Stack` and `Ages::Table` are types of their instances. If another module also imports `Stack(int)`, it gets the same instance and the same types.

An instance can be imported after the types it uses, in the same file. This `Demo` uses the module `Lookup` above and exits with status 31:

```
type Person is record
  name: String;
  age: int;
end;

import People is Lookup(String, Person, stringEqual);

proc start(): int
  var people: People::Table;
  var ann: Person;
begin
  ann.age := 31;
  People::put(people, "ann", ann);
  return People::get(people, "ann").age;
end;
```

## 12. Open decisions

**Settled:** `and`/`or` short-circuit; `div`/`mod` are Euclidean; `char` is a 32-bit Unicode code point; reuse is through generic modules with opaque type parameters, one instance per program (4.4), and through procedure parameters; every `ref` is nullable; procedures are top-level only; type compatibility is by name; exceptions carry no data and there is no `finally`; uncaught exceptions print the name, procedure, task, failing expression and call chain to standard error and exit with status 1; `#` is the comment character and there is no not-equal glyph; string literals are fresh arrays; functions' results are never discarded implicitly (`void` discards); evaluation is left to right; aliasing of by-reference arguments is allowed and means true aliasing, with assignments in program order (6); `fixed` has arbitrary precision, division gives the sum of the scales, divisions and `strFixed` are rounding steps that round once when they are the whole right-hand side (7.4), and number text uses `-` and `.` (9.2); `Overflow` and `LostPrecision` are used as described in 7.5; concurrency is by tasks in lockstep rounds (section 8) with copy-only messages, a full queue failing its sender with `OutOfMemory`, the step budget and memory limit as compiler options, and the size and step tables in 8.4; tasks reach each other through links (mailboxes passed only at `spawn`, so dependencies cannot form a cycle), replies, which take one answer or a stated number, and read-only views (8.2, 8.10, 8.12); a task ends in a fixed order, children first and its own deferred calls last, with `cancel`, waves, the `Closed` exception and a backstop (8.11); failures are written to the report, raised in the parent at `sync`, `await` and `awaitChange`, and make the exit status 1 when nobody observed them, except for disowned children (8.6); files, listeners and publishers move only at `spawn`; views are charged to their owner and to every reader in full (8.12); copying is priced at one step per object plus one per 16 units (8.4); `Net` provides sockets as pairs of `File` halves, with timeouts and replay (9.7); enumerations are plain lists of `int` constants; servers use a tagged record; the entry point is `start`, in one of four forms, with command-line arguments from `args()` and an optional `int` exit status (9.1); I/O is UTF-8 text plus bytes, with blocking reads that complete at round boundaries, `nil` at end of input, and distinct exceptions for file failures (9.2, 9.3); `share` declares read-only values built before round 1 in declaration order, imports first, with writes caught at run time by `ReadOnly` and initializers that may read input but not write or start tasks (4.2); the library is the module `std`, imported automatically, usable unqualified, and hidden by a module's own declarations except for `len` and the task, handle and view calls (4.1); modules are source files compiled together, with `import`, `Module::name` qualification, `private` hiding and acyclic imports (4.1); there are no mutable global variables, and read-only data is shared with `share`; a type or procedure may refer to itself, and mutually recursive types or procedures are declared in an `also` group (4.3); loops have `exit` and `next`, and there is no `case`; literals use backslash escapes and hexadecimal integers, bit operations are library procedures; source is UTF-8 with ASCII identifiers; time, sleep and random are outside events (9.6); tasks end by returning, by failing or by being closed with `cancel` (8.11); files have `exists`, `delete`, `rename`, `mkdir` and sorted `list`; `assert` is a library procedure; `defer` schedules clean-up calls (5.1); a handler may name several exceptions; `/` on two `int`s is an error; the loss-of-digits exception is `LostPrecision`; wrapping and rotate operations exist (9.5); record fields can be `private`; procedures stay second-class; named number types do not mix (7.4); `fixed` has explicit-scale `divide` and `rescale` with named rounding modes, whole-part and unit conversions, and exact powers (7.4, 9.5); the compiler options and their defaults are in 8.9; there is no program-controlled `exit(n)`, only the status returned by `start`; an instance import may stand among the declarations (4.4); a call can be followed by selectors (6); parameters and local variables hide library names like top-level declarations do (4.1).

**Deferred to later versions:**

1. **Standard collections.** The generic-module mechanism (4.4) is in place. The library's own lists, maps, sets and heaps, written as generic modules and shipped with the implementation, are not specified yet.
2. **Character classification and case mapping** (`isDigit`, `isLetter`, `toUpper` and so on).
3. **Shell interop**: running other programs, pipes, environment variables.
4. **More file operations**: `seek`, `tell`, `size`, and a non-blocking check for pending input.
5. **Signal handling by programs** and timers that deliver messages instead of blocking. A signal now closes the program in an orderly way (8.11), but programs cannot react to it.
6. **Lossy push (taps).** A weak, droppable notification from a task to tasks that do not depend on it. Views cover state, and a log kept by the owner and asked for with a reply covers events that must not be lost.
7. **Views inside published graphs**, so that a directory of views could itself be published. Views now travel in messages and `spawn` arguments only.
8. **Regions for published data** and a limit on versions that charges readers one unit per reference, for large read-mostly tables read by many tasks.
9. **TLS and datagrams** for `Net`, and a `select` that waits on several sources at once.
10. **Checkpoint and restore**, and running independent groups of tasks without a global round barrier.
11. **Worker pools that receive connections.** A connection is owned by one task from the `accept`.
12. **The values of the constants.** The copy price, `--grace`, `--grace-time`, `--socket-buffer` and `--handles` have defaults (8.9), to be set from measurement.
