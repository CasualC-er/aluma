# Alúma Style Guide

This guide sets the rules for how programs are written. The compiler accepts other layouts. Programs in this style are written the same way everywhere.

## 1. Files and modules

1. One module per file. The file is named after the module, with `.alm`.
2. Module names start with a capital letter and capitalise each word: `IntList`, `Stats`.
3. A file lists its parts in this order, with one blank line between parts:
   1. `import` line for plain modules (one line, names in alphabetical order)
   2. `import` lines for instances whose arguments are only library names and imported names (one line each, in alphabetical order of the instance names)
   3. constants
   4. enumerations
   5. types
   6. exceptions
   7. procedures, instance imports and shared values, each after the declarations it needs and before the first declaration that uses it
4. Procedures are ordered so that every procedure comes after the ones it calls. Helpers marked `private` come first. In the main module, `start` is last. The members of an `also` group come together, in the order a reader needs them.
5. A module marks `private` everything that other modules do not need, including record fields that must keep a rule between them.
6. A module does not hide library names, except for a good reason that is written in a comment. A variable or parameter does not hide one either.
7. An instance import whose arguments name types, constants or procedures of the same file goes right after the last of them. A shared value goes right after the last procedure its initializer calls. Both are written on their own, with one blank line before and after.

## 2. Layout

1. Indent with 2 spaces. Never use tabs.
2. Write one statement per line.
3. Lines are at most 80 characters long.
4. A procedure is laid out like this:
   - `proc` line at the left edge,
   - `var` lines indented 2 spaces,
   - `begin` at the left edge,
   - statements indented 2 spaces,
   - `end;` at the left edge.
5. A block (`if`, `elsif`, `else`, `while`, `try`, `catch`) puts its statements 2 spaces deeper than its keyword. `elsif`, `else`, `catch` and `end` line up with the keyword that opened the block.
6. A long parameter list or argument list continues on the next line, lined up under the first item after the opening parenthesis.
7. Leave one blank line between top-level declarations that are more than one line long. Do not leave more than one blank line anywhere.
8. Files are UTF-8, end with a line break, and have no spaces at the end of a line.

```
proc largest(values: ref array of int): int
  var best: int;
  var i: int;
begin
  best := values[0];
  i := 1;
  while i < len(values) do
    if values[i] > best then
      best := values[i];
    end;
    i := i + 1;
  end;
  return best;
end;
```

## 3. Spacing

| Write | Not |
|---|---|
| `a := b + c;` | `a:=b+c;` |
| `f(x, y)` | `f( x,y )`, `f (x, y)` |
| `a[i]`, `node.key` | `a [i]`, `node . key` |
| `x: int` | `x : int`, `x:int` |
| `IntList::push(l, 1)` | `IntList :: push(l, 1)` |
| `_5` | `_ 5` |
| `a := b;` | `a := b ;` |

- Put one space on each side of `:=`, `is`, and every binary operator (`+ - * / = < <= > >= ~ div mod and or`).
- Put one space after `not`.
- Put one space after `,` and `;` when text follows on the same line.
- Put no space just inside parentheses or brackets.

## 4. Names

| Kind | Style | Examples |
|---|---|---|
| variables, parameters, fields | lower camel case | `total`, `lineCount`, `reply` |
| procedures and functions | lower camel case | `countWords`, `push` |
| types | capitalised words | `Node`, `Request` |
| exceptions | capitalised words | `NotFound`, `Empty` |
| constants, enumeration names, shared values | capitalised words | `Size`, `Get`, `Squares` |
| modules | capitalised words | `IntList` |
| type parameters of a generic module | one capital letter, or capitalised words | `T`, `K`, `V`, `Key` |
| instances | capitalised words naming the use, not the type | `Names`, `Ages` |

1. Names are words. Use short names (`i`, `n`) only for loop counters and for values whose meaning is clear from a few lines around them.
2. Procedures that do something are verbs: `push`, `countWords`. Functions that answer a question start with `is` or `has` joined to the next word: `isEmpty`, `hasKey`.
3. Exceptions name what went wrong: `NotFound`, `Empty`. They do not end in `Error`.
4. The names of one enumeration start with the same word when a module has more than one enumeration: `ColorRed`, `ColorBlue`, `ShapeSquare`.
5. A task's inbox parameter is called `queue`. A handle or mailbox variable is named after the task it reaches: `worker`, `store`. A mailbox parameter that leads to a task says so: `toRoom`. The reply field of a request is called `back`. A view or a publisher is named after what it shows: `feed`, `rates`.
6. A message record type is named for its job: `Request`, `Reply`, `Job`, `Result`.
7. A variable or parameter is never named like a library procedure, such as `units`, `floor`, `min`, `list` or `split`.

## 5. Comments

1. A comment starts with `#` and one space.
2. A comment on its own line is indented like the code it describes and goes above it.
3. A comment after code has two spaces before the `#`.
4. Each procedure that other modules use has a comment above it that says what it does, and which exceptions it raises.
5. Comments say what a value means, what a unit is, or why code is the way it is. They do not repeat the code.
6. Do not leave code in comments.
7. Check an assumption with `assert`, not with a comment.

```fragment
# Returns the number of non-empty words in text.
# Raises NilReference if text is nil.
proc countWords(text: String): int
...
```

## 6. Declarations and procedures

1. Use a constant for every number that has a meaning. `0`, `1` and `_1` can stay as they are.
2. List variables at the start of the procedure, one per line, in the order they are first used.
3. Rely on zero values when zero is the intended start. Do not write `x := 0;` only to say so. Write it when the value is reset later in a loop.
4. Parameters are passed by reference. A procedure assigns to a parameter only when the caller expects its variable to change, and says so in the comment above it.
5. To walk a list or tree that was passed in, copy the reference into a local variable and move the local.
6. Keep a procedure to what fits on one screen (about 40 lines). Split it when it does two jobs.
7. Write `void` only when throwing away a result is intended.
8. Return early for bad input, at the top of the procedure.

## 7. Expressions

1. Put parentheses around each side of a mixed `and` / `or`: `(a and b) or c`.
2. Write the test for "not equal" as `not (a = b)`, with parentheses. Write the test for "not the same object" as `not (a ~ b)`.
3. Compare with `nil` using `~`: `if node ~ nil then`.
4. Write a `bool` as it is: `if found then`, not `if found = true then`.
5. Use `div` and `mod` for whole numbers. Use `/` only for `fixed`.
6. Write a negative number as `_5`, and put no space after `_`.
7. Do not rely on aliasing: do not pass the same variable twice to a procedure that assigns through both.
8. Select from a call result (`People::get(people, "ann").age`) to read a single field or element. When the result is needed twice, keep it in a variable.

## 8. Loops

1. Use a loop condition when it says what the loop does: `while i < len(a) do`.
2. Use `while true do` with `exit;` for loops that end in the middle, such as reading until `nil`.
3. Use `next;` only to remove a level of nesting. A loop where `next;` skips the step that advances the counter is not allowed: advance first.
4. Do not change a loop counter anywhere except at one place in the body.

## 9. Exceptions

1. Declare an exception in the module that detects the problem, and raise it there.
2. Catch only the exceptions you can handle. List them by name: `catch NotFound, Empty do`.
3. Use `catch do` (no name) only to clean up and then `raise;`, or in the top procedure of a task to report a failure.
4. Do not use exceptions for ordinary events. The end of input is `nil`. A missing key is an answer, not an exception.
5. Name the exceptions a procedure can raise in the comment above it.

```
exception TooBig;

proc check(n: int)
begin
  if n > 100 then
    raise TooBig;
  end;
end;

proc start(): int
begin
  try
    check(500);
  catch TooBig, NilReference do
    writeLine("rejected");
  end;
  return 0;
end;
```

## 10. Resources

1. Write the `defer` on the line right after the call that gets the resource.
2. Close a file in the procedure that opened it.
3. Do not use `return` to skip a clean-up. Everything that must happen is in a `defer`.
4. Close a listener and a socket half in the task that owns it. A half that was given to another task is closed there.

```
proc firstLine(path: String): String
  var f: File;
begin
  f := open(path, ModeRead);
  defer close(f);
  return readLine(f);
end;
```

## 11. Decimal and whole numbers

1. Give each kind of quantity its own named type, even when the digits are the same: `type Dollars is fixed(12,2);`. Do not use `int` for money.
2. Write the scale of every `fixed` type on purpose. A reader should be able to tell what the smallest unit is.
3. Round in one place per statement. When the way of rounding matters, use `divide` or `rescale` with a mode. Use `rounded` only when the right side is a single division, `divide`, `rescale` or `strFixed` call, or a single multiplication.
4. Use `units` and `fromUnits` only where a count of smallest units meets the outside world (files, ids, protocols).
5. Do not use `wrapAdd`, `wrapSub`, `wrapMul` or the rotates except in hashes and random number generators. Write the reason in a comment.
6. Write `0xFF`-style literals for bit masks, and decimal literals for everything else.

```
type Dollars is fixed(12,2);

# Splits an amount into three parts. The last part is what is left.
proc split3(amount: Dollars, first: Dollars, last: Dollars)
begin
  first := divide(amount, 3, 2, RoundDown);
  last := amount - first - first;
end;
```

## 12. Tasks

1. Give each piece of changeable data to one task. Other tasks ask that task for it by message. If many tasks only read it, the owner publishes it as a view.
2. A task that waits for work uses `await`. Do not write a loop that calls `receive` and then `sync()` only to wait.
3. A request record has the fields every kind needs, a `kind` field, and a `back` field of type `reply of R` if it wants an answer. The kinds are named by an `enum`. A request that is answered in parts makes its reply with `replyTo(queue, n)`.
4. Keep messages small. Sending takes time in proportion to what is copied.
5. Put large data that is known at the start and never changes in a `share` value. Publish data that changes, and remember that every reader is charged for the whole version.
6. Pass a mailbox only as an argument of `spawn`, derived with `link(h)` or `self(queue)`. Look at `failed(h)` or `ended(h)` before handing out a link, so that a failed service is noticed before its clients are.
7. Do not write a `Stop` request. Close a task down with `cancel(h)`, or let its parent finish. A task that loops on `await` ends when `Closed` is raised there.
8. Do not catch `Closed`. A catch-all does not stop it, because it is raised again at the next waiting point. List the exceptions a task expects by name.
9. Put a `defer` in a task body for anything the task must release when it ends, on the line after the call that gets it, and before the loop that uses it.
10. Keep the deferred calls of a closing task short. They cannot start tasks or accept connections. Give a blocking call in them a time limit.
11. A parent that depends on a child looks at `failed(h)` in its loop or calls `awaitEnd(h)`. A parent that does not need a child's failure, such as a server and its clients, calls `disown` on the handle.
12. Catch what a dropped connection causes (`Net::ConnectionReset` and `IOError`) in the task that owns the socket. A client that goes away is not a failure.
13. Use one task for each connection, and two when the directions are independent, one for each half.
14. An owner publishes once per round, after all its changes. A reader calls `awaitChange` in a loop and reads `latest` once for each wake-up. It ends the loop by catching `DeadTask`.
15. Let `start` have an inbox only when other tasks need to send to it.

```
enum Add, Read;

type Answer is record
  total: int;
end;

type Request is record
  kind: int;
  amount: int;
  back: reply of Answer;
end;

proc counter(queue: inbox of Request)
  var request: ref Request;
  var result: ref Answer;
  var total: int;
begin
  while true do
    await(queue, request);
    if request.kind = Add then
      total := total + request.amount;
    elsif request.kind = Read then
      result := new(Answer);
      result.total := total;
      answer(request.back, result);
    end;
  end;
end;
```

## 13. Tests

1. A test is a main module that imports the module under test, checks results with `assert`, and returns 0 from `start`.
2. A test module is named after the module it tests, with `Test` at the end: `IntListTest`.
3. One `assert` checks one thing.

```
import IntList;

proc start(): int
  var l: IntList::List;
begin
  IntList::push(l, 4);
  assert(IntList::pop(l) = 4);
  return 0;
end;
```

## 14. Groups and generic modules

1. Use an `also` group only for real mutual recursion. A procedure that could call a helper declared before it does not belong in a group.
2. Keep a group small: the members must fit on one screen together, or be split into a smaller group and plain helpers.
3. Put the `generic` lines first, in this order: types, constants, procedures. Leave one blank line after them.
4. Ask a generic module for one `generic proc` per operation it needs, and no more. Do not pass a procedure that the module does not call.
5. Give a generic module a noun for a name: `Stack`, `Lookup`, `Queue`.
6. Name an instance for what it is used for: `import Names is Lookup(String, int, stringEqual);`, not `StringIntLookup`.
7. Write each instance on an `import` line of its own, so that lines stay short.
8. Import an instance in each module that uses it. The same arguments always give the same instance, so values can be passed between modules.
9. When an instance needs a type of the same file, write the `import` line right after that type, as the first thing that follows it.
10. In a generic module, keep the fields that hold its rules `private`.

```
import Names is Lookup(String, int, stringEqual);

type Person is record
  name: String;
  age: int;
end;

import People is Lookup(String, Person, stringEqual);

proc start(): int
  var ages: Names::Table;
  var people: People::Table;
  var ann: Person;
begin
  Names::put(ages, "ann", 31);
  ann.age := Names::get(ages, "ann");
  People::put(people, "ann", ann);
  return People::get(people, "ann").age;
end;
```
