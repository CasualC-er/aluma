# Alúma Programming Guide

This guide describes Alúma: how to write programs in it and what each part does. Source files end in `.alm`.

## 1. Two small programs

A program is one or more source files. One of them is the main module, and it contains a procedure named `start`. The program runs `start`.

```
proc start(): int
begin
  writeLine("Hello");
  return 0;
end;
```

This prints `Hello` and a line break. The number that `start` returns is the exit status of the program.

This program reads lines from standard input, treats each line as a whole number, and prints the total:

```
proc start(): int
  var line: String;
  var total: int;
begin
  line := readLine();
  while not line ~ nil do
    total := total + strInt(line);
    line := readLine();
  end;
  writeLine(concat("total ", intStr(total)));
  return 0;
end;
```

Things to notice:

- A procedure has a name, a parameter list in parentheses, an optional result type after `:`, an optional list of `var` declarations, and a body between `begin` and `end`.
- Every statement ends with `;`.
- `:=` assigns. `~` asks whether two references are the same object. `nil` is the reference that points to nothing.
- Every variable starts with a zero value: `total` starts at 0, and `line` starts as `nil`.
- `readLine()` gives `nil` when there is no more input.

## 2. Source text

**Files.** A source file is UTF-8 text. Its name, without `.alm`, is the name of its module.

**Spacing.** Spaces, tabs and line breaks separate words and have no other meaning.

**Comments.** `#` starts a comment. It runs to the end of the line.

```fragment
total := total + 1;   # add one
```

**Names.** A name is made of ASCII letters and digits and starts with a letter. Capital and small letters are different letters. Underscores are not allowed. A name cannot be a keyword or `std`. A name can be the same as a name in the standard library; see section 10.

**Keywords.** These words are reserved:

```keywords
also and array begin bool catch char const defer div do else elsif end enum exception exit false fixed generic if import inbox int is mailbox mod new nil next not of or private proc publisher raise record ref reply return rounded share task then true try type var view void while
```

**Statements.** Every statement is followed by `;`, including the last one before `end`.

## 3. Literals

| Kind | Written as | Examples |
|---|---|---|
| whole number | digits, or `0x` and hexadecimal digits | `42`, `0xFF` |
| decimal number | digits, `v`, digits | `12v50`, `0v05`, `5v0` |
| character | `'`, one character, `'` | `'a'`, `'\n'` |
| string | `"`, characters, `"` | `"hello"`, `"line\n"` |
| truth value | `true` or `false` | |
| no reference | `nil` | |

**Whole numbers.** A negative number is written with `_` in front: `_5` is minus five. A whole-number literal larger than the largest `int` is an error. The one exception is 9223372036854775808 written as `_9223372036854775808`, which is the smallest `int`.

**Decimal numbers.** The `v` is the decimal point. The number of digits after `v` is the scale of the literal: `12v50` has scale 2, and `5v0` has scale 1.

**Escapes.** In characters and strings, a backslash starts an escape:

| Escape | Meaning |
|---|---|
| `\n` | line feed |
| `\t` | tab |
| `\r` | carriage return |
| `\0` | the character with code 0 |
| `\\` | backslash |
| `\'` | single quote |
| `\"` | double quote |
| `\u{h}` | the character with hexadecimal code `h` (one to six digits) |

Any other escape is an error. A literal cannot contain a line break. A character literal holds exactly one character.

**Strings.** Each time a string literal is evaluated, it makes a new `String` that can be changed.

## 4. Types

### 4.1 Basic types

| Type | Values |
|---|---|
| `int` | whole numbers from -9223372036854775808 to 9223372036854775807 |
| `bool` | `true` and `false` |
| `char` | one Unicode character, stored as its code point |
| `fixed(p, s)` | exact decimal numbers with at most `p` digits, of which `s` come after the decimal point |

`ord(c)` gives the code of a character, and `chr(n)` gives the character with code `n`. `chr` raises `InvalidCharacter` if `n` is not a valid code point.

### 4.2 Arrays

`array [n] of T` is a row of `n` values of type `T`. The length `n` must be a constant. Positions are numbered from 0. Using a position below 0 or at or above the length raises `IndexRange`. The function `len(a)` gives the length of any array.

A variable of an array type holds the array itself. Assigning it to another variable copies all elements.

```fragment
var a: array [5] of int;
```

`ref array of T` is a reference to an array whose length is chosen while the program runs. `new(array of T, n)` makes one with `n` elements, all zero. A negative `n` raises `NegativeSize`.

```fragment
var b: ref array of int;
b := new(array of int, 3);
b[0] := 7;
```

Assigning a `ref array of T` variable copies the reference, so both variables refer to the same array.

### 4.3 Records

A record is a group of named fields.

```
type Point is record
  x: int;
  y: int;
end;
```

A variable of type `Point` holds the fields itself. `p.x` is the field `x` of `p`. Assigning a record copies all its fields.

### 4.4 References

`ref Point` is a reference to a record of type `Point`. `new(Point)` makes a new record with all fields zero and gives a reference to it.

```fragment
var p: ref Point;
p := new(Point);
p.x := 3;
```

- A reference can be `nil`. Using a field or element through `nil` raises `NilReference`.
- Assigning a reference copies the reference. Both variables then refer to the same record.
- A reference can only point to a record or to a run-time array. It cannot point to an `int`, `bool`, `char` or `fixed` value.
- A record can contain a reference to its own type. This makes lists and trees:

```
type Node is record
  key: int;
  left: ref Node;
  right: ref Node;
end;
```

Objects that nothing refers to any more are freed automatically.

### 4.5 Strings

`String` is a name in the standard library for `ref array of char`. A string is an array of characters, so `len(s)` is the number of characters and `s[0]` is the first one. Strings can be changed in place. Two strings are compared with `stringcmp(a, b)`, which gives -1, 0 or 1 by comparing character codes from the start. `concat(a, b)` makes a new string.

### 4.6 Zero values

Everything starts as zero: an `int` is 0, a `bool` is `false`, a `char` has code 0, a `fixed` is 0, a `ref`, `mailbox`, `reply`, `view`, `publisher`, `task`, `File` or `Listener` is `nil`. Records and arrays start with every part zero.

### 4.7 Named types

`type Name is T;` gives a type a name. Two types are the same only if they are the same declared type. Two records with the same fields but different names are different types, and values of one cannot be assigned to the other. Every `record` declaration makes its own type.

### 4.8 Named number types

`type Dollars is fixed(12,2);` and `type Id is int;` make named number types. A value of a named type carries that name. Plain `int` and `fixed` values and literals carry no name.

- `+`, `-` and the comparisons accept two values with the same name, or one named and one plain value. Two different names are an error. The result of `+` or `-` keeps the name.
- `*`, `/`, `div`, `mod`, `divide`, `rescale`, `floor` and the other operations accept any names and give a plain result.
- A plain value, or a value with the same name, can be assigned to a variable of the named type. A named value can be assigned to a plain variable. A named value cannot be assigned to a variable with a different name.
- An argument that is a variable must have exactly the type of its parameter. Any other argument follows the assignment rule.

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

`price + fare` and `total := fare` are errors. The program exits with status 23.

### 4.9 Copying and sharing

| Assigning a value of this type | does this |
|---|---|
| `int`, `bool`, `char`, `fixed` | copies the value |
| record, fixed-length array | copies all parts |
| `ref` | copies the reference |

## 5. Expressions

### 5.1 Operators

From loosest to tightest:

| Level | Operators |
|---|---|
| 1 | `or` |
| 2 | `and` |
| 3 | `not` |
| 4 | `=` `<` `<=` `>` `>=` `~` |
| 5 | `+` `-` |
| 6 | `*` `/` `div` `mod` |
| 7 | `_` (minus sign) |
| 8 | literals, names, calls (with optional selectors), `new`, parentheses |

- At levels 5 and 6, operators of the same level group from left to right.
- `not` and `_` can be repeated: `not not a`, `_ _ 5`.
- Comparisons do not chain. `a < b < c` is an error. Write `a < b and b < c`.
- There is no "not equal" operator. Write `not a = b`. It means `not (a = b)`. Write `not a ~ b` for "not the same object".
- `and` and `or` evaluate the left side first. They evaluate the right side only when the left side has not decided the answer. In `(j >= 0) and (a[j] > x)`, the element `a[j]` is never read when `j` is negative.
- All evaluation goes from left to right: the parts of an operator, the arguments of a call, and, in an assignment, the indexes on the left side before the right side.
- A call can be followed by `[index]` and `.field`: `args()[0]`, `People::get(people, "ann").age`. The selection applies to the result. A `nil` result raises `NilReference`, and an index out of range raises `IndexRange`. A selection from a call is a value only: it cannot be assigned to, and as an argument it is a temporary.

### 5.2 Equality and identity

- `=` compares values. It works on `int`, `bool`, `char` and `fixed`, and on records and fixed-length arrays that contain no references.
- `~` asks whether two references (or two mailboxes, or two views) are the same object. `nil` can be on either side.
- `=` does not work on references. To compare the contents of strings, use `stringcmp(a, b) = 0`.
- `<`, `<=`, `>` and `>=` work on numbers. To order characters, compare `ord(c)` values.

### 5.3 Whole-number arithmetic

`+`, `-`, `*`, `div`, `mod` and `_` work on `int`. If a result does not fit in an `int`, in either direction, `Overflow` is raised.

`div` and `mod` use the rule `a = (a div b) * b + (a mod b)` with `0 <= a mod b < |b|`. The remainder is never negative.

| Expression | Value |
|---|---|
| `7 div 2` | 3 |
| `7 mod 2` | 1 |
| `_7 div 2` | -4 |
| `_7 mod 2` | 1 |
| `7 div _2` | -3 |
| `7 mod _2` | 1 |
| `_7 div _2` | 4 |
| `_7 mod _2` | 1 |

A zero divisor raises `DivideByZero`. Dividing the smallest `int` by -1 raises `Overflow`.

### 5.4 Decimal arithmetic

A `fixed(p, s)` value is an exact decimal number. `fixed(7,2)` holds numbers from -99999.99 to 99999.99 in steps of 0.01. There is no limit on `p`.

Operations on `fixed` values give exact results, however many digits they need:

| Operation | Result scale |
|---|---|
| `a + b`, `a - b` | the larger of the two scales |
| `a * b` | the sum of the two scales |
| `a / b` | the sum of the two scales |

- `0v10 + 0v20` is exactly `0v30`.
- Comparisons use the numeric value: `1v5 = 1v50` is true.
- An `int` can be used with `fixed` values. It counts as a `fixed` with scale 0. Dividing by an `int` keeps the scale of the dividend.
- The symbol `/` is for `fixed` values. For two whole numbers use `div`: `7 / 2` is an error.
- Dividing by zero raises `DivideByZero`.

**Division.** The result has the sum of the scales. `6v00 / 2v00` is `3v0000`. `1v00 / 4v00` is `0v2500`. `1v00 / 3v00` is not exact at scale 4, so non-zero digits would be lost. That raises `LostPrecision`, unless the statement uses `rounded`, as described next.

**Assignment checks the size.** When a value is assigned to a variable of type `fixed(p, s)`:

- If the value has too many digits before the decimal point, `Overflow` is raised.
- If non-zero digits after the decimal point would be lost, `LostPrecision` is raised.
- Zero digits that are lost cause no error. Missing digits are filled with zeros.

**Rounding.** Writing `rounded` after `:=` makes the statement round half away from zero instead of raising `LostPrecision`:

```fragment
tax := rounded price * 0v19;
```

The places where digits can be lost are a division, a call of `strFixed`, a call of `divide` or `rescale` without a rounding mode, and the assignment itself. If the right side of a `rounded` assignment is just one division, or one `strFixed`, `divide` or `rescale` call without a mode, it is rounded once, to the smaller of its own scale and the scale of the variable. In any other expression, each of these rounds at its own scale, and the assignment rounds again.

**Choosing the scale yourself.** Two calls state the wanted scale in the code. The scale `k` is a constant of 0 or more.

- `divide(a, b, k)` is the exact quotient of `a` and `b`, rounded once to scale `k`, whatever the scales of `a` and `b` are. `divide(1v0, 8v0, 3)` is `0v125`. Dividing by zero raises `DivideByZero`.
- `rescale(x, k)` is `x` with scale `k`. Zeros are added when `k` is larger. An `int` counts as scale 0.

With no mode, these two calls follow the rules above: lost non-zero digits raise `LostPrecision`, and `rounded` makes them round half away from zero.

**Rounding modes.** `divide` takes a fourth argument and `rescale` a third: a constant naming how to round.

| Mode | Rounds |
|---|---|
| `RoundHalfAway` | to the nearest, ties away from zero |
| `RoundHalfEven` | to the nearest, ties to the even last digit |
| `RoundDown` | toward zero |
| `RoundUp` | away from zero |
| `RoundFloor` | toward minus infinity |
| `RoundCeiling` | toward plus infinity |

A call with a mode rounds as the mode says. It never raises `LostPrecision`, and `rounded` does not change it.

| Expression | Value |
|---|---|
| `rescale(2v675, 2, RoundHalfEven)` | `2v68` |
| `rescale(_2v5, 0, RoundFloor)` | `_3` |
| `divide(1v00, 3v00, 4, RoundHalfAway)` | `0v3333` |

Assigning a `fixed` value to an `int` variable follows the same rule.

## 6. Statements

**Assignment.** `x := e;` or `x := rounded e;`. The left side can be a variable, a field, or an array element: `a[i].x := 3;`.

**Procedure call.** `name(arguments);`

**Function call with the result thrown away.** `void name(arguments);` A function's result is never discarded without `void`. Calling a function as a plain statement is an error. Using `void` on a procedure with no result is an error.

**If.**

```fragment
if a < b then
  ...
elsif a = b then
  ...
else
  ...
end;
```

There can be any number of `elsif` parts, and the `else` part is optional.

**While.**

```fragment
while condition do
  ...
end;
```

**Exit and next.** `exit;` leaves the innermost `while`. `next;` skips the rest of the body of the innermost `while` and tests the condition again. Both are errors outside a `while`. They stay inside one procedure.

**Defer.** `defer name(arguments);` schedules a call for when the procedure exits. See section 9.3.

**Return.** `return;` ends a procedure. `return e;` ends a function and gives the value `e`.

**Raise.** `raise Name;` raises an exception. See section 9.

**Try.** `try ... catch Name do ... end;` See section 9.

## 7. Declarations

These are written at the top level of a file:

```
const Size is 100;
enum Get, Put, Delete;
type Node is record key: int; left: ref Node; right: ref Node; end;
exception NotFound;
```

- **Constants.** `const Name is expression;` The expression is worked out when the program is compiled. A constant can be used wherever a constant is needed, for example `array [Size] of int` or `fixed(Digits, 2)`.
- **Enumerations.** `enum A, B, C;` declares the `int` constants `A` = 0, `B` = 1, `C` = 2. They are plain `int` values. They have no type of their own.
- **Types.** `type Name is T;`
- **Exceptions.** `exception Name;`
- **Shared values.** See section 12.
- **Procedures.** See section 8.

Rules:

- A name must be declared before it is used, except among the members of a recursive group (section 7.1).
- A type can refer to itself through `ref`, and a procedure can call itself.
- Variables are declared only at the start of a procedure, before `begin`. There are no variables at the top level of a file.
- Procedures are declared at the top level only. A procedure cannot contain another procedure.
- All names in a module share one set of names: constants, enumeration names, types, variables, procedures, exceptions and imported modules cannot repeat a name.

### 7.1 Recursive groups

Types that refer to each other, or procedures that call each other, are declared as a group. A declaration that starts with `also` joins the group of the declaration before it.

```
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
```

```
type Tree is record
  label: int;
  kids: ref Forest;
end;

also type Forest is record
  trees: ref array of ref Tree;
end;
```

- All members of a group are types, or all are procedures. A member can start with `also private`.
- Inside a group, every member can be used in every member, including in parameter types and field types. After the last member, the names are available as usual.
- `also` cannot come first in a file, after any other kind of declaration, or after a declaration of the other kind.
- A record or fixed-length array can hold another member of its group by value only if that member does not hold it back. Otherwise it must use `ref`. A circle of containment, or an alias circle such as `type A is B; also type B is A;`, is an error.

## 8. Procedures

```fragment
proc name(a: int, b: ref Node): int
  var t: int;
begin
  ...
  return t;
end;
```

- The result type after the parameter list is optional. A procedure with a result type is a function.
- Local variables are listed with `var` before `begin`. Each starts with its zero value.

### 8.1 Parameters

Every parameter is passed by reference. The parameter is another name for the caller's variable. Assigning to the parameter changes the caller's variable.

```
proc swap(a: int, b: int)
  var t: int;
begin
  t := a;
  a := b;
  b := t;
end;
```

After `swap(x, y)`, the variables `x` and `y` have exchanged values.

If an argument is not a variable, for example `5` or `x + 1`, its value is put into a temporary variable, and the parameter is another name for the temporary.

```
proc push(head: ref Cell, x: int)
  var c: ref Cell;
begin
  c := new(Cell);
  c.value := x;
  c.link := head;
  head := c;
end;
```

Here `head := c;` changes the caller's variable. To walk a list without changing the caller's variable, copy the reference into a local variable first.

The same variable may be passed for two parameters. The two parameters are then the same variable, and the assignments through them take effect in program order, so the result is the same everywhere. It can still be confusing to read, so avoid it.

### 8.2 Procedure parameters

A parameter can be a procedure. Its type is written `proc(types): result`:

```
proc apply(f: proc(int): int, x: int): int
begin
  return f(x);
end;

proc double(n: int): int
begin
  return n * 2;
end;

proc start(): int
begin
  return apply(double, 21);
end;
```

The argument is the name of a procedure whose parameter types and result type are exactly the same. A procedure parameter is called like any procedure and can be passed on to another procedure. It cannot be assigned, stored in a variable or field, or returned.

### 8.3 Recursion

A procedure can call itself. Calls can nest up to a fixed depth (10,000 by default). Going deeper raises `StackOverflow`.

## 9. Exceptions

An exception stops the current work and moves to the nearest handler.

**Declaring.** `exception TooBig;` An exception carries no data.

**Raising.** `raise TooBig;`

**Handling.**

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
  catch TooBig do
    writeLine("too big");
  catch do
    writeLine("something else");
  end;
  return 0;
end;
```

- The statements between `try` and the first `catch` run first.
- If one raises an exception, the handlers are checked in order. The first `catch` whose name matches runs its statements. Then the program continues after `end`.
- A handler can list several names: `catch TooBig, TooSmall do` runs for either one.
- A `catch do` with no name matches any exception. It must be the last handler.
- If no handler matches, the exception leaves the procedure and continues in the caller.
- Inside a handler, `raise;` with no name raises the exception being handled again.
- If the try statements finish without an exception, no handler runs.

**Cleaning up.** There is no `finally`. To release something such as a file, use `defer` (section 9.3). To react to a failure, catch it, and write `raise;` to pass it on.

### 9.1 Predefined exceptions

| Exception | Raised when |
|---|---|
| `Overflow` | an `int` or `fixed` result is out of range, in either direction |
| `LostPrecision` | non-zero decimal digits would be lost in an assignment without `rounded` |
| `DivideByZero` | `div`, `mod` or `/` has a zero divisor |
| `IndexRange` | an array position is below 0 or at least the length |
| `NegativeSize` | a size, count or shift that must not be negative is negative (`new(array of T, n)`, `repeat`, `power`, `shl`, `shr`, `sleep`), or `random(n)` has `n` less than 1 |
| `NilReference` | a field or element is used through `nil`, or a message is sent to a `nil` mailbox |
| `OutOfMemory` | a task's live size would go over the memory limit, or a sent message does not fit in the receiver's limit, or a view's version no longer fits |
| `StackOverflow` | calls are nested deeper than the depth limit |
| `DeadTask` | a message is sent to a task that ended in an earlier round; a reply is used a second time; `spawn` is given a link to a task that is closing down or has ended; `awaitChange` is called on a view whose owner has ended and has nothing newer |
| `Deadlock` | every live task is waiting and nothing can wake any of them |
| `Closed` | a task that is closing down reaches a waiting point, or a task that is cancelled or finished starts new work |
| `Timeout` | a read, `accept` or `connect` with a time limit does not finish in time, or a closing task has used up its grace time |
| `InvalidCharacter` | `chr(n)` is given a number that is not a Unicode code point |
| `FormatError` | text is not in the form a library procedure needs (a number, or a non-empty separator) |
| `BadEncoding` | bytes read as text are not valid UTF-8, or text written holds a value that is not a code point |
| `FileNotFound` | `open`, `delete` or `list` names a file or directory that does not exist |
| `PermissionDenied` | the operating system refuses access to a file |
| `ReadOnly` | a field or element of a shared value is assigned |
| `BadContext` | an operation that is not allowed while shared values are built is called |
| `AssertFailed` | `assert(c)` is called with `c` false |
| `IOError` | any other file or stream failure, including use of a closed file and a failed write that is reported later |

### 9.2 Uncaught exceptions

If an exception leaves `start`, the program writes a report to standard error and exits with status 1:

```text
uncaught Overflow in Stats::sum (task 1)
  evaluating: total + x
  called from: Stats::average
  called from: App::start
```

- The first line names the exception, the procedure where it was raised, and the task.
- The second line shows the source text of the expression or statement that was running (for `raise`, the `raise` statement).
- The other lines list the calling procedures, innermost first.
- Procedures and declared exceptions are shown with their module name, such as `IntList::Empty`. Predefined exceptions are shown without one.
- An exception while shared values are built (section 12) shows `(initialization of Module::Name)` instead of the task number.
- When another task fails, the same report is written at the end of the round, with `failed` in place of `uncaught` and a last line `  parent: task N`. Section 11.5 says what happens to the failure next.

### 9.3 Deferred calls

`defer name(arguments);` records a call. The call runs when the procedure that contains the `defer` exits, whether by `return`, by reaching the end, or because an exception leaves it.

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

- To defer a function call, write `defer void name(arguments);`.
- When `defer` runs, arguments that are not variables are worked out and kept. Arguments that are variables are passed by reference as usual, so the call sees their values at the time it runs.
- When the procedure exits, the recorded calls run in the reverse order of recording. A `return e;` works out `e` first, then the deferred calls run, then control goes back to the caller.
- A `defer` can be anywhere in a procedure except inside a `while`.
- `exit` and `next` leave a loop and not the procedure, so they run no deferred calls.
- If a deferred call raises an exception, the other deferred calls still run. The procedure then exits with the exception of the last call that failed, in place of any exception that was already leaving.
- A task body is a procedure. Its own deferred calls run when the task ends, after everything the task started has ended (section 11.8). A task that is stopped by the backstop runs none of them.
- The `defer` statement costs 1 step. A deferred call costs what a normal call costs when it runs.

## 10. Modules

A module is a source file. The module `Tree` is the file `Tree.alm`.

**Main module.** The compiler is given one main module. It must contain `start`. No other module may contain `start`. The program is the main module and every module it imports, directly or through others.

**Importing.** `import A, B;` goes at the top of a file, before every other declaration. The names of `A` are then written `A::name`. An instance of a generic module can also be imported later in the file (section 10.1).

```
import IntList;

proc start(): int
  var l: IntList::List;
begin
  IntList::push(l, 1);
  return IntList::pop(l);
end;
```

- `Module::name` can be a constant, an enumeration name, a type, an exception, a shared value or a procedure. It can be used in any place where an unqualified name is allowed. It cannot be assigned to.
- There is one level of qualification: `A::b` is allowed, `A::B::c` is not.
- Imports do not pass on. If `A` imports `B`, a module that imports `A` must import `B` to use its names.
- A module cannot import itself, and modules cannot import each other in a circle. The compiler reports the circle.
- `private` before a declaration hides it from other modules. It can go before `const`, `enum`, `type`, `exception`, `share` and `proc`. For `enum`, it hides all the names. Using a private name from another module is an error. A record field is visible unless its declaration starts with `private`. A private field can be read and assigned only in the module that declares the record type, and a use anywhere else is an error. Other modules can still hold, pass and copy whole records with private fields, and variables and `new` give zeroed records. A private type can appear in the signature of a visible procedure. Other modules can pass its values around but cannot write its name.
- Each module has its own names. Two modules can both declare `Node` or `Empty`. `A::Node` and `B::Node` are different types, and `A::Empty` and `B::Empty` are different exceptions.
- The standard library is the module `std`. Every module imports it automatically, so no `import` line is needed. Its names can be written alone, as in `len(a)`, or with the module, as in `std::len(a)`.
- A declaration at the top level of a module hides a library name with the same spelling, in that module only, wherever it is used. Using the name before its declaration is an error. `std::name` always reaches the library one. A parameter or a local variable hides a library name in the same way, inside its procedure. The name `len` and the task, handle and view calls (`spawn`, `send`, `receive`, `await`, `sync`, `self`, `replyTo`, `answer`, `link`, `cancel`, `awaitEnd`, `ended`, `failed`, `closing`, `disown`, `newView`, `publish`, `latest`, `version` and `awaitChange`) cannot be hidden.
- The compiler looks for the file `A.alm` in the directories given by the `--path` option, in order. The first match is used.

A complete example in two files. Module `IntList`:

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

The program prints `empty` and exits with status 2. `IntList::drop` is private, so `App` cannot call it, and `App` cannot read or assign the private fields `head` and `size` of `IntList::List`.

A module that hides a library name:

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

Inside this module `max` is the procedure above, and `std::max` is the library one. The program prints `pear` and exits with status 9.

### 10.1 Generic modules

A generic module is written once and used with different types, constants and procedures. Its file starts with `generic` lines, one for each parameter:

| Line | Parameter |
|---|---|
| `generic type T;` | a type |
| `generic const N: int;` | a constant `int` |
| `generic proc equal(a: K, b: K): bool;` | a procedure with this signature (the names are only for reading) |

The `generic` lines come before every `import` and declaration. A generic module cannot be the main module and cannot contain `start`.

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

Module `Lookup`, which takes a procedure:

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

Using them:

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

The program exits with status 42.

**Importing an instance.** `import Name is Generic(arguments);` makes an instance called `Name`. Plain modules and instances can share one `import` line. The arguments follow the order of the `generic` lines:

- a type for each `generic type`;
- a constant expression for each `generic const`;
- the name of a procedure for each `generic proc`. Its parameter types and result type must be exactly those of the declared signature, with the type parameters replaced by the given types. The built-in procedures that accept several types (section 14.6) cannot be given.

**Importing an instance among the declarations.** An instance can also be imported on a line of its own after the declarations it uses. Its arguments can then name types, constants and procedures declared above it in the same file. The name `Name` can be used from that line on. The line takes neither `private` nor `also`, and an `also` declaration cannot follow it. Imports of plain modules stay at the top of the file.

This `Demo` uses `Lookup` from above and exits with status 31:

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

The names of the instance are written `Name::member`. A generic module cannot be imported without arguments, and an ordinary module cannot be given any.

**One instance per program.** The same generic module with the same arguments is one instance, wherever it is imported. Arguments are the same when they are the same type, equal constants, or the same procedure. `Numbers::Stack` is then one type, and a stack made in one module can be passed to another module that imports `Stack(int)`. Different arguments make different instances, with different types and exceptions.

**What generic code can do with a type parameter.** A value of type `T` can be assigned, passed, stored in fields and array elements, and made as a zero value, for example with `new(array of T, n)`. It cannot be compared with `=`, put in order, used in arithmetic, compared with `nil`, indexed, or have fields selected. `=` is also not allowed inside the module on a record or array that has a part of type `T`. Everything else the module needs from a type comes from its `generic proc` parameters. A generic procedure parameter is called like any procedure and can be passed on, but it cannot be assigned or stored. A constant parameter can be used wherever a constant is allowed, such as `array [N] of T`.

**Other rules.**

- A type containing `T` that is used as a message type or as the type of a shared value is checked at the import: it must not contain an `inbox` or a `File`.
- A generic module can import other modules and instantiate other generic modules, with its own parameters as arguments. Instantiations can nest 32 deep. A longer chain is an error.
- Each instance has its own types, exceptions, shared values and procedures.

## 11. Tasks

A task is a separate line of work with its own memory. A program can have many tasks. Tasks do not share changeable memory. They talk by sending messages, and they share data that changes by publishing read-only views of it.

A task can reach another task in three ways:

- through a **link**, a mailbox that it is given when it is started;
- through a **reply**, an address for one answer, which comes inside a request;
- through a **view**, which it can read.

Only a link makes one task depend on another when the program closes down (section 11.8).

### 11.1 The pieces

- The program starts with one task, the initial task. It has number 1 and runs `start`. Other tasks are made with `spawn`.
- Every task has its own heap. Records and arrays that a task makes belong to that task. No reference points from one task's heap into another's. (Shared values, section 12, and published views, section 11.9, are the exceptions. They cannot be changed.)
- The program has no changeable global variables. All changeable data belongs to a task.
- A task can have an **inbox**. The type `inbox of R` is the receiving end of its queue. Only the task that owns the inbox can use it. `R` is a record type, and it is the type of the messages.
- A **mailbox** is the sending end of an inbox. The type `mailbox of R` can be copied and stored inside a task. A task can pass a mailbox to a new task when it starts it, and in no other way. A mailbox is never sent in a message. A mailbox that is `nil` raises `NilReference` when it is used for sending.
- Every task except the first has a **parent**, the task that started it. A task cannot outlive its parent.

### 11.2 Starting a task

A task body is a procedure. If its first parameter has type `inbox of R`, the task has an inbox. `spawn(body, arguments...)` starts a task that runs `body`:

- The inbox parameter is the new task's own inbox. The other parameters get the arguments.
- `spawn` gives back a **handle** to the new task: a `task of R` if the body has an inbox of `R`, and a `task` if it has no inbox. Section 11.7 says what a handle can do.
- The new task starts in the round after the one in which `spawn` was called.
- Plain data is copied, as in a message. A `reply` or a `view` is passed as it is.
- A parameter of type `mailbox`, `File`, `Listener` or `publisher` must get an argument of exactly that type, not a record that contains one. A mailbox lets the new task send to another task. A `File`, a `Listener` or a `publisher` must be a variable, and it is **moved**: the new task owns it, and the variable becomes `nil`.

A task ends when its body returns or an exception leaves it, and everything it started has ended. Section 11.8 gives the details.

### 11.3 Messages and replies

A message is a reference to a record of type `R`.

- `send(mb, m)` sends the message `m` to the mailbox `mb`. A `task of R` handle can be used in place of the mailbox. `send` copies the record and everything it refers to, at that moment. Later changes by the sender do not change the message. An object that can be reached in several ways is copied once, and circles are kept.
- References into shared values (section 12) and into views (section 11.9) are copied as references. Everything else is copied in full.
- Messages are always copied. A message cannot contain an `inbox`, a `mailbox`, a `publisher`, a `task`, a `File` or a `Listener`. It can contain a `reply` or a `view`.
- A sent message can be received from the start of the next round.
- `receive(inbox, m)` takes the oldest message that can be received and puts it in the variable `m`. It gives `true`. If there is none, it gives `false`, and `m` does not change. It never waits.
- `await(inbox, m)` does the same, but if there is no message it waits until a round in which there is one.
- `self(inbox)` gives a mailbox for the task's own inbox. To let a child task reach its parent, pass `self(inbox)` to `spawn`.
- A message does not say who sent it.
- Sending to a task that ended in an earlier round raises `DeadTask`. Sending to a task that has not ended succeeds, and a message still in an inbox when its task ends is thrown away.
- Queued messages take up room in the receiving task's memory. If a message does not fit, it is thrown away, and the sender gets `OutOfMemory` the next time it returns from `sync` or `await`.

**Replies.** A request that wants an answer carries a `reply of R`. The task that asks makes it with `replyTo(inbox)`, and the task that answers uses it with `answer(r, m)`:

- A reply made with `replyTo(inbox)` works once. After the first `answer`, any later `answer` through the same reply, or through a copy of it, raises `DeadTask`. So does an `answer` to a task that has ended.
- `replyTo(inbox, n)` makes a reply that takes up to `n` answers, for a request that is answered in parts, such as the fills of an order. `n` less than 1 raises `NegativeSize`. A reply makes no link, so the answerer needs no mailbox to the requester.
- If several answers to one reply come in a round, they are taken in the order of the answering tasks' numbers, and then of sending, until the answers left are used up. The others are dropped, and their senders' next return from `sync` or `await` raises `DeadTask`.
- Answers count against the requester's queue. A requester that stops receiving makes the answerer fail with `OutOfMemory`, as with `send`, so an answerer that serves many clients, some of them perhaps slow, should publish a view instead.

**A complete example.** A worker doubles numbers. `start` asks it to double 21:

```
type Result is record
  value: int;
end;

type Job is record
  value: int;
  back: reply of Result;
end;

proc doubler(queue: inbox of Job)
  var job: ref Job;
  var out: ref Result;
begin
  while true do
    await(queue, job);
    out := new(Result);
    out.value := job.value * 2;
    answer(job.back, out);
  end;
end;

proc start(queue: inbox of Result): int
  var worker: task of Job;
  var job: ref Job;
  var result: ref Result;
begin
  worker := spawn(doubler);
  job := new(Job);
  job.value := 21;
  job.back := replyTo(queue);
  send(worker, job);
  await(queue, result);
  writeLine(intStr(result.value));
  return 0;
end;
```

The program prints `42`. `start` has an inbox, and the job carries a reply for it, which the worker uses to answer. The job travels in round 1, the worker answers in round 2, and `start` prints in round 3. When `start` returns, `doubler` is closed down (section 11.8).

**Several kinds of request.** An inbox takes one message type. To accept several kinds of request, use one record with an `int` field for the kind, named with an `enum`, and the fields every kind needs:

```
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
```

The compiler does not check which fields go with which kind. A field that was not set is zero.

### 11.4 Rounds

Tasks run in numbered rounds, starting at round 1.

- In each round, every task that can run does run, from where it last stopped, until one of these happens:
  - it calls `sync()`, which stops it for the rest of the round (it runs again in the next round);
  - it waits in `await`, `awaitChange`, `awaitEnd`, `sleep` or a read or a write that has to wait;
  - it uses up its step budget (section 11.6);
  - it ends.
- Tasks in the same round can run in any order, or at the same time. The result is the same.
- A task that waits does not run and uses no steps. It runs again in the first round in which what it waits for is ready.
- When all tasks have stopped, the round ends. Then, in this order:
  1. Output that tasks wrote during the round is written, task by task, in order of task number.
  2. Tasks that have gone past their grace limit are stopped (section 11.8).
  3. Tasks that have ended are recorded as ended, and their views are marked as ended.
  4. New tasks get their numbers.
  5. Messages sent during the round are put into their inboxes, ordered by the number of the sending task and then by the order in which they were sent.
  6. New versions of views become visible (section 11.9).
  7. Cancels and signals take effect, and tasks that are being closed down move to their next step (section 11.8).
  8. Failures are passed on (section 11.5).
  9. Events from outside the program take effect: new input, connections, the end of a sleep or a time limit.
  10. The next round starts.

A message therefore takes one round to arrive. Two tasks that answer each other in turn make one exchange per round. An answer that goes through a third task takes two.

### 11.5 Failure and waiting forever

- If an exception leaves a task, the task fails. At the end of that round, the failure is written to standard error, in the form of an uncaught exception but with the word `failed` and a last line that names the parent task.
- The failure is then given to the parent, but only if the parent is not being closed down, the parent has no other failure waiting, and the child has not been disowned (section 11.7). If several children fail in the same round, the lowest task number is the one that is given.
- A failure that was given to the parent is raised in the parent at its next `sync`, `await` or `awaitChange`. `awaitEnd(h)` raises the failure of its own child. No other call raises a child's failure: not `spawn`, not a read, not `sleep`.
- A failure that nobody saw is **undelivered**. That means it was never raised in the parent, and the parent did not look at it with `failed(h)` or `awaitEnd(h)`. If a program would exit with status 0 and a failure is undelivered when it ends, the exit status is 1 instead. A child that was disowned is left out.
- If a deferred call raises while a task is being closed down, its exception replaces the one on its way out, and the task fails. When the parent is also being closed down, that failure is undelivered, so a failed clean-up at shutdown sets the exit status to 1.
- If, at the end of a round, every task that is alive is waiting in `await`, `awaitChange` or `awaitEnd`, no message is on its way, nothing outside the program is awaited, and no closing down is under way, then `Deadlock` is raised in the lowest-numbered task, which is the initial task unless it has finished. A task that is sleeping or waiting for a read, a connection or a time limit is waiting for something outside, so it does not cause `Deadlock`. A task that stopped because of `sync()` or its budget can still run, so it does not cause `Deadlock` either.

### 11.6 Steps and memory

Each task has a step budget for each round and a memory limit. Both are counted in abstract units, not in time or bytes.

**Steps.**

| Event | Steps |
|---|---|
| a simple statement (assignment, call, `return`, `raise`, `defer`) | 1 |
| testing the condition of `if`, `elsif` or `while` | 1 each time |
| entering a procedure | 1 |
| leaving a procedure because of an exception, per procedure | 1 |
| `fixed` addition or subtraction | the size units of the larger operand |
| `fixed` multiplication or division | the product of the size units of the operands |
| `new`, `send`, `answer` | the copy price of the objects made or copied |
| `spawn` | 1 plus the copy price of the copied arguments |
| `publish` | 1 plus the copy price of the graph |
| other task, handle, view and `Net` operations | 1 each |
| `write` | the size units of the string |
| `readLine` | the size units of the result |
| each byte read or written | 1 |
| whole-number arithmetic, comparisons, indexing | 0 |

The **copy price** of some objects is 1 step for each object and 1 step for each 16 size units, rounded up. A reference into a shared value or a view counts as one unit and is not followed.

At the start of each round, a task's budget grows by `N` (the `--steps` option), up to a top value of `N`. A task runs only while its budget is above zero. When the budget reaches zero, the task stops for the rest of the round, as if it had called `sync()`. With the option `--strict-steps`, the task gets the exception `OutOfSteps` instead of stopping, and no debt is carried into later rounds. A program that runs without `OutOfSteps` then gives the same results for every `--steps`, which is what a simulation wants.

An operation is never cut in half. If an operation costs more than the budget that is left, it finishes, and the budget goes below zero. The task then does not run until the budget is above zero again. Example: with a full budget `N`, an operation that costs 2.5 `N` leaves -1.5 `N`. The next round gives -0.5 `N`, and the task does not run. The round after gives 0.5 `N`, and it runs.

**Size units.**

| Value | Units |
|---|---|
| `int`, `bool`, `char`, reference, mailbox, reply, publisher, task, `File`, `Listener` | 1 |
| `view` | 1; after its owner has ended, 1 plus the units of its last version |
| `fixed(p, s)` | one for every 18 digits of `p`, rounded up |
| record | 1 plus the sum of its fields |
| fixed-length array of `n` values of `T` | `n` times the size of `T` |
| `ref array of T` | 1 plus `n` times the size of `T` |

The live size of a task is the total of the units of the distinct objects in its own heap that can still be reached from its variables, counted once each, together with the versions it can reach (section 11.9). A tree node with a key and two children is 4 units. A string of 1000 characters is 1001 units. Shared values are not counted, and a reference to one is 1 unit. Messages waiting in the inbox are counted.

If an allocation would take the live size over the memory limit, `OutOfMemory` is raised. A task can also get `OutOfMemory` at a round boundary, when a message arrives that does not fit, or when the owner of a view ends and the last version is more than the task can hold. It is then raised the next time the task returns from `sync` or `await`.

### 11.7 Handles and links

`spawn` gives back a handle. It belongs to the parent task. The parent can keep it in variables and records, but cannot send it, pass it to `spawn`, or put it in a shared or published value.

| Call | What it does |
|---|---|
| `send(h, m)` | sends `m` to the child's inbox (only for a `task of R`) |
| `link(h)` | gives a `mailbox of R` for the child's inbox, to pass to `spawn` (only for a `task of R`) |
| `cancel(h)` | asks the child to close down (section 11.8) |
| `awaitEnd(h)` | waits until the child has ended, and raises its failure if it failed |
| `ended(h)`, `closing(h)` | tell whether the child has ended, or is being closed down |
| `failed(h)` | tells whether the child ended by failure, and counts as looking at the failure |
| `disown(h)` | the parent will not look at the child's failure |

**Links.**
- A task gets a link to another task by being given a mailbox when it is started, with `link(h)` or `self(inbox)` as the argument. This is the only way. A task that starts another can pass on a mailbox it holds.
- The tasks that were given a link to a task Y, and the tasks they gave it to, are the **holders** of Y. Y itself does not count as a holder, and neither does the parent that called `link(h)`.
- `spawn` raises `DeadTask` if a mailbox argument is a link to a task that is being closed down or has ended. A task that is closing down takes no new clients.
- Every holder of Y was started, directly or not, by Y's parent. So `cancel` never reaches a task that the caller did not start.
- A holder always has a higher task number than the task it holds a link to, and a child always has a higher number than its parent. Closing down waits only for tasks with higher numbers, so the waits cannot go in a circle.

**Disowning.** A child that you expect to fail now and then, such as one task for each client of a server, can be disowned: `disown(spawn(handler, input, output))`. Its failure is written to the report, but it is never given to the parent and does not change the exit status.

**Restarting a service.** A task that fails on its own leaves the tasks that hold a link to it with a link to a task that has ended. Links cannot be handed over again, so the way to restart a service is to close down its clients as well and start them again. `cancel(h)` on the handle of a service that has ended still closes down its holders. Cancelling a service that is shared closes down its clients.

### 11.8 Ending and closing down

A task ends in a fixed order: everything it started ends first, and then its own deferred calls run. The same order is used when a task is cancelled, when it fails, when it returns, and when the program ends.

- A task is **finished** when its body has returned or an exception has left it. A finished task waits for its children to end, then runs the deferred calls of its body procedure, and ends. A task that has no children ends at once.
- `cancel(h)` closes down a child, together with the tasks that hold a link to it and all of their children. These tasks are **cancelled**. A cancelled task goes on running while the tasks it waits for close down. A server keeps serving its clients until they have gone.
- A cancelled task is **closing** when all its children have ended and all its holders have ended. A finished task is closing when its children have ended. Every task that becomes closing in the same round does so together, in a **wave**. The waves decide the round in which each deferred call runs, and so the order of output.
- A closing task gets the exception `Closed` at its next **waiting point**: `sync`, `await`, `awaitChange`, `awaitEnd`, `sleep`, or any read, write, `accept` or `connect` that has to wait. The frames unwind, and the deferred calls run. The deferred calls of the body run last.

Some details:

- `Closed` is sticky. A handler that catches it does not clear it, and it is raised again at the next waiting point. A waiting point inside a deferred call never raises `Closed`, so clean-up can wait.
- `Closed` wins. At `await` it is raised even if a message can be received.
- A task that ends by `Closed` leaving its body has ended normally. It has not failed.
- A cancelled or finished task cannot **start new work**. `spawn` and `Net::accept` raise `Closed` in it, and a task that is waiting in `Net::accept` is woken. This `Closed` is an ordinary exception: a handler can catch it, and it does not come back. `Net::connect` is allowed, so a task that is closing can still send its last words to another service.
- If a task does not end after `--grace` rounds of running, or after `--grace-time` milliseconds of waiting, since it started closing, the **backstop** acts. When the time is up, each waiting call raises `Timeout`. When the rounds are up, the task is stopped without running its remaining deferred calls, and the runtime closes its files, listeners and publishers.
- When the initial task finishes, the whole program closes down by these rules, and the program ends when the initial task has ended. When the operating system asks the program to stop, the initial task is cancelled, and the exit status is 130 or 143. A second request ends the program at once.

Here is the order for the chat server of section 13.4, after a signal. The boundary where the signal takes effect is B0:

```text
B0       start is cancelled, and so are the rooms, sessions and writers below it.
         start was waiting in accept: it gets Closed there.
         The writers have no children and no holders, so they are closing.
round 1  The writers get Closed and end. start leaves its loop.
B1       The sessions have no children left: they are closing.
round 2  The sessions get Closed in their reads and end.
B2       The room has no holders left: it is closing.
round 3  The room gets Closed and ends.
B3       start has no children left: it is closing.
round 4  start closes the listener in its own deferred call and ends.
B4       The program ends.
```

### 11.9 Published views

A view lets one task publish data that many tasks read, without copies or queues. The owner holds a `publisher of R`. Everyone else holds a `view of R`. `R` is a record type.

| Call | What it does |
|---|---|
| `newView(pub, v)` | makes a pair and puts its ends in the variables `pub` and `v` |
| `publish(pub, root)` | makes what `root` refers to the new version |
| `latest(v)` | a read-only reference to the current version, or `nil` before the first one |
| `version(v)` | the number of the current version, of type `Version`; 0 means none |
| `awaitChange(v, seen)` | waits until the version number is above `seen`, and gives it |
| `ended(v)` | tells whether the owner has ended |

- `publish` takes a snapshot, so the owner can go on changing its own copy. The new version becomes visible at the end of the round. If a task publishes several times in one round, only the last one counts.
- A version cannot be changed. A write through a reference into one raises `ReadOnly`.
- A published value cannot contain a mailbox, a reply, a view, an inbox, a publisher, a task, a `File` or a `Listener`.
- The `view` is passed to other tasks in a message or as a `spawn` argument. The `publisher` is moved to the task that will publish, as a `spawn` argument.
- If the owner ends, the last version stays readable, and `ended` is true. When a reader has seen the last version, `awaitChange` raises `DeadTask`. A reader that missed versions can tell how many: `new - seen - 1`.
- A view does not lose events. A reader that falls behind sees the latest state, not every change. To read every event, ask the owner for "everything from number `n`" with a reply, and keep the log in the owner.

**Cost.** The owner pays for the current version, and every reader pays for everything it can reach in a version, as if it held a copy. The real memory is shared. `publish` costs the copy price of what is published, so a ledger of 100,000 records of 10 units each costs 162,500 steps for one `publish`. While the version is current, the owner holds its own copy and the published one. After the owner ends, every task that holds the view pays for its last version.

**An example.** `ticker` publishes a counter three times. `watcher` waits for changes and reports the last value:

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

The program prints `last 3`. After `spawn(ticker, pub)`, the variable `pub` is `nil`, because the publisher was moved. `ticker` publishes in rounds 2, 3 and 4 and ends in round 5. `watcher` ends its loop in round 6, when `awaitChange` finds that the owner has ended and nothing newer exists.

## 12. Shared values

A shared value is read-only data that every task can read without copying.

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

- The form is `share Name: T is expression;` at the top level of a file. `private` can go before it. Other modules write `Module::Name`.
- `T` cannot contain an `inbox`, `mailbox`, `reply`, `view`, `publisher`, `task`, `File` or `Listener`.
- **Building.** Before round 1, the shared values are built one at a time. The compiler visits the main module and its imports depth first, in the order of the `import` lines, each module once. Inside a module, shared values are built from the top down, and an instance imported among the declarations is visited at its line, between the shared values above it and those below it. An initializer can use shared values built before it, including those of imported modules.
- Each initializer runs as task 0 with its own heap. Its result is copied into the shared area, the way a message is copied. Objects reached in several ways are copied once, and circles are kept. The copy is read-only.
- An initializer can call any procedure. It can also call `args()`, `now()`, `random(n)`, and these reading operations: `open` for reading, `readLine`, `readBytes`, `close`, `stdin`, `exists` and `list`. These finish at once.
- An initializer that calls `write` or any other output operation, `open` for writing, `delete`, `rename`, `mkdir`, `sleep`, `sleepRounds`, any call of `Net`, or any task, handle or view call (`spawn`, `send`, `receive`, `await`, `sync`, `self`, `replyTo`, `answer`, `link`, `cancel`, `awaitEnd`, `disown`, `newView`, `publish` and `awaitChange`) raises `BadContext`.
- If an exception leaves an initializer, or it goes over `--init-steps`, `--init-memory` or `--shared-memory`, the program ends with status 1 before round 1.
- **Reading.** Every task can read a shared value. A reference into the shared area can be stored, passed on, compared with `~` and sent in a message. It is sent as a reference, not copied.
- **Writing.** Assigning to a field or element of an object in the shared area raises `ReadOnly`. Such a reference has the same type as any other reference.
- A shared name is not a variable, so it cannot be assigned to. If its type is a value type (`int`, a record, a fixed-length array), its fields and elements cannot be assigned either. Passing it for a parameter makes a temporary copy.

## 13. Input and output

### 13.1 Starting and ending

`start` has one of four forms:

| Form | Result |
|---|---|
| `proc start()` | exit status 0 |
| `proc start(): int` | the returned number is the exit status (0 to 255, otherwise `Overflow`) |
| `proc start(queue: inbox of R)` | exit status 0; the task has an inbox |
| `proc start(queue: inbox of R): int` | the returned number is the exit status; the task has an inbox |

- Before round 1, the shared values are built. Then `start` runs in round 1.
- The program ends when `start` has ended, which is after every task it started has closed down (section 11.8).
- An exception that leaves `start` gives the report in section 9.2 and exit status 1.
- `args()` gives the command-line arguments, without the program name, as a `ref array of String`.
- If the operating system asks the program to stop, the initial task is cancelled and the program closes down in the order of section 11.8, with status 130 (interrupt) or 143 (terminate). A second request ends the program at once. A program cannot handle signals, but it can be closed down.

### 13.2 Text

Text is UTF-8.

- `write(s)` writes the string `s` to standard output. `writeLine(s)` also writes a line break. `writeErr` and `writeErrLine` do the same for standard error.
- `readLine()` gives the next line of standard input as a `String`, without its end of line (`\n` or `\r\n`). It gives `nil` at the end of the input.
- Output that a task writes is held until the end of the round and then written in task order. Standard output and standard error are separate.
- If a line is ready at the start of a round, `readLine()` returns in that round. Otherwise the task waits until a round in which a line is ready. While a task waits for a line and standard input is open, `Deadlock` cannot be raised.
- If several tasks wait for standard input, the task with the lowest number gets the line.
- To wait for either a line or a message, read lines in a separate task that sends each line as a message.

**Numbers as text.**

| Call | Result |
|---|---|
| `intStr(n)` | the digits of `n`, with `-` in front if negative |
| `fixedStr(x)` | the digits of `x`, with all its decimal places and `.` as the decimal point: `-1.50`, `0.00`, `7` |
| `charStr(c)` | a string of one character |
| `strInt(s)` | the whole number written in `s` |
| `strFixed(s, k)` | the number written in `s`, as an exact `fixed` with scale `k` (a constant) |

Number text is ASCII digits with an optional `-` in front. For decimal numbers, a `.` with at least one digit on each side. There is no `+`, no space, no exponent. Leading zeros are allowed. Text that does not fit raises `FormatError`. `strInt` raises `Overflow` for a number that is too large.

`strFixed` fills in zeros when the text has fewer than `k` decimal digits: `"1.5"` with `k = 2` is `1v50`. If the text has more digits, the extra digits are lost. If any of them is not zero, `LostPrecision` is raised, unless the statement uses `rounded`.

There is no formatting language. Build output with `intStr`, `fixedStr`, `charStr`, `concat`, `padLeft`, `padRight` and the other string procedures.

### 13.3 Files

| Call | Meaning |
|---|---|
| `open(path, mode)` | opens the file named by the string `path`. `mode` is `ModeRead`, `ModeWrite` (empties the file or creates it) or `ModeAppend`. Gives a `File`. |
| `close(f)` | closes the file and writes out what is waiting |
| `stdin()`, `stdout()`, `stderr()` | the task's files for the standard streams |
| `readLine(f)`, `write(f, s)`, `writeLine(f, s)` | the text calls above, on any file |
| `readBytes(f, n)` | up to `n` bytes as a `ref array of int` with values 0 to 255, or `nil` at the end of the file |
| `readLine(f, timeout)`, `readBytes(f, n, timeout)` | the same reads, but they raise `Timeout` if nothing comes within `timeout` milliseconds |
| `writeBytes(f, a)` | writes the elements of `a` as bytes. A value outside 0 to 255 raises `Overflow`. |
| `exists(path)` | `true` if a file or directory has that name |
| `delete(path)` | deletes a file or an empty directory |
| `rename(from, to)` | renames a file or directory; an existing file named `to` is replaced |
| `mkdir(path)` | makes one directory; its parent must exist |
| `list(path)` | the names in a directory as a `ref array of String`, sorted by character code, without `.` and `..` |

- `File` is a standard type. Its zero value is `nil`. Using `nil` raises `NilReference`.
- A `File` belongs to one task. It cannot be sent in a message. It can be given to another task only as an argument of `spawn`, which moves it: the variable of the sender becomes `nil`. When a task ends, its files, including the ones it was given, are closed in the order they were opened.
- Text calls and byte calls can be mixed on one file. Text calls read and write UTF-8, and byte calls do not.
- A read, `open`, `exists`, `delete`, `rename`, `mkdir` or `list` makes the task wait until it is finished. That is at least one round.
- A write is held and written at the end of the round. If the write fails then, `IOError` is raised at the task's next call on that file, or at `close`.
- Using a file after `close` raises `IOError`.
- Failures raise `FileNotFound` (the file or directory is missing), `PermissionDenied`, or `IOError` (anything else, including `mkdir` on a name that exists).

### 13.4 Sockets

Sockets are in the module `Net`, which the implementation provides. Write `import Net;` to use it.

| Call | Meaning |
|---|---|
| `Net::listen(port)` | starts listening on the `int` port and gives a `Net::Listener`. Raises `Net::AddressInUse` if the port is taken. |
| `Net::accept(l, input, output)` | waits for a connection and puts its two halves, both `File`s, in the variables `input` and `output`. |
| `Net::connect(host, port, input, output)` | opens a connection to the string `host` and the port, and puts its two halves in `input` and `output`. |
| `Net::close(l)` | stops listening. |
| `readLine(f, timeout)`, `readBytes(f, n, timeout)` | the reads of section 13.3, but they raise `Timeout` if nothing comes within `timeout` milliseconds. |

`accept` and `connect` also take a time limit as a last argument. The other exceptions are `Net::ConnectionRefused`, `Net::ConnectionReset`, `Net::Unreachable` and `IOError`.

- A connection is two `File`s, one to read and one to write, and all the calls of section 13.3 work on them. A half belongs to one task and moves to another task only as an argument of `spawn`. A `Net::Listener` moves the same way.
- A read waits until data or the end of the input comes. `readLine` gives `nil` at the end.
- A write goes into a buffer of `--socket-buffer` bytes and is sent at the end of the round. If the buffer is full, the write waits, so a slow client stops only the task that writes to it.
- A task cannot wait for a socket and for its inbox at once. When the two directions of a connection are independent, as in a chat, use two tasks: one owns the input half, the other the output half.
- When a run is repeated from its recording, the network is not touched. The data that was read comes from the recording, and what is written is dropped.

**A chat server.** Every line that a client sends goes to all the clients. `room` keeps the last 64 lines and publishes them as a view. Each connection has a `session` that reads lines from the client and a `writer` that writes new lines to it.

```
import Net;

const Keep is 64;

exception RoomFailed;

type Line is record
  text: String;
end;

type Log is record
  head: int;
  lines: array [Keep] of String;
end;

proc room(queue: inbox of Line, pub: publisher of Log)
  var log: ref Log;
  var msg: ref Line;
begin
  log := new(Log);
  while true do
    await(queue, msg);
    log.lines[log.head mod Keep] := msg.text;
    log.head := log.head + 1;
    while receive(queue, msg) do
      log.lines[log.head mod Keep] := msg.text;
      log.head := log.head + 1;
    end;
    publish(pub, log);
  end;
end;

proc writer(feed: view of Log, output: File)
  var seen: Version;
  var log: ref Log;
  var cursor: int;
begin
  defer close(output);
  seen := 0;
  cursor := 0;
  try
    while true do
      seen := awaitChange(feed, seen);
      log := latest(feed);
      if log.head - cursor > Keep then
        cursor := log.head - Keep;
      end;
      while cursor < log.head do
        writeLine(output, log.lines[cursor mod Keep]);
        cursor := cursor + 1;
      end;
    end;
  catch DeadTask, Net::ConnectionReset, IOError do
  end;
end;

proc session(toRoom: mailbox of Line, feed: view of Log,
             input: File, output: File)
  var msg: ref Line;
  var text: String;
begin
  defer close(input);
  void spawn(writer, feed, output);
  try
    text := readLine(input);
    while not (text ~ nil) do
      msg := new(Line);
      msg.text := text;
      send(toRoom, msg);
      text := readLine(input);
    end;
  catch Net::ConnectionReset, IOError do
  end;
end;

proc start(): int
  var listener: Net::Listener;
  var input: File;
  var output: File;
  var pub: publisher of Log;
  var feed: view of Log;
  var roomH: task of Line;
begin
  listener := Net::listen(9000);
  defer Net::close(listener);
  newView(pub, feed);
  roomH := spawn(room, pub);
  while true do
    Net::accept(listener, input, output);
    if failed(roomH) then
      raise RoomFailed;
    end;
    disown(spawn(session, link(roomH), feed, input, output));
  end;
  return 0;
end;
```

```text
start ─ owns the listener; made the view; moved the publisher into room
 ├─ room         publisher: appends lines to the log and publishes once per round
 ├─ session #1   link to room, the view, the input half
 │    └─ writer  the view, the output half
 ├─ session #2   ...
```

- A line takes two rounds to go from one client to the others. A session sends it to `room` in round r. The room publishes it in round r + 1. The writers wake in round r + 2 and write it. `room` takes everything that has arrived and publishes once per round.
- A writer that falls more than 64 lines behind skips the lines it missed. A chat can accept that. A feed that must not lose events asks the owner for "everything from number `n`" with a reply.
- `start` checks `failed(roomH)` after each `accept`, before it hands out a link to the room. If the room has failed, `start` raises `RoomFailed` and the program closes down. A failed session is different: it is disowned, so it is written to the report and `start` goes on. A client that drops its connection is not a failure, because `session` and `writer` catch the exceptions that a drop causes.
- The `defer` that closes the listener is before the loop. A `defer` after the loop would never be recorded.
- When the program receives a signal, it closes down in the order shown in section 11.8. The writers end first, then the sessions, then the room, and `start` closes the listener last.
- If the room fails, the sessions cannot reach it any more. The way to start again is to cancel the room's handle, which also closes down the sessions, and then start a room and accept new connections.

## 14. Standard library

Names in this section belong to the module `std`. They are available in every module, alone or as `std::name`, and a module can hide them with its own declarations (section 10).

### 14.1 General

| Name | Meaning |
|---|---|
| `len(a)` | length of any array |
| `ord(c)`, `chr(n)` | character to code, code to character |
| `String` | the type `ref array of char` |
| `File` | the file type |
| `stringcmp(a, b)` | -1, 0 or 1 by comparing character codes |
| `concat(a, b)` | a new string with `a` followed by `b` |
| `assert(c)` | raises `AssertFailed` if the `bool` `c` is false. The report shows the source text of `c`. |
| `args()` | the command-line arguments |
| `stringEqual(a, b)` | `true` if the strings have the same length and characters |
| `hashInt(n)`, `hashString(s)` | a non-negative `int` made from the value alone, the same on every run and in every implementation: 64-bit FNV-1a over the 8 bytes of `n` (most significant first), or over the UTF-8 bytes of `s`, with the top bit cleared |

### 14.2 Strings

Every result is a new string. A `nil` string raises `NilReference`.

| Name | Meaning |
|---|---|
| `substring(s, start, n)` | the characters from index `start`, at most `n` of them. `start` outside 0 to `len(s)` raises `IndexRange`. A negative `n` raises `NegativeSize`. |
| `find(s, t, from)` | the lowest index at or after `from` where `t` occurs in `s`, or -1. `from` outside 0 to `len(s)` raises `IndexRange`. An empty `t` is found at `from`. |
| `startsWith(s, t)`, `endsWith(s, t)` | `bool` |
| `join(parts, sep)` | the strings of the `ref array of String` `parts` with `sep` between them |
| `split(s, sep)` | a `ref array of String` with the pieces of `s` between the occurrences of `sep`. It always has at least one element. Neighbouring separators give empty pieces. An empty `sep` raises `FormatError`. |
| `repeat(s, n)` | `n` copies of `s` joined together. A negative `n` raises `NegativeSize`. |
| `padLeft(s, width, c)`, `padRight(s, width, c)` | `s` with the char `c` added on the left or right until it has `width` characters. A copy of `s` if it is already long enough. |
| `trim(s)` | `s` without leading and trailing space, tab, line feed and carriage return |

Example:

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

`split("a  b c", " ")` gives `"a"`, `""`, `"b"`, `"c"`. `next` skips the empty piece. The program exits with status 15.

### 14.3 Numbers

| Name | Meaning |
|---|---|
| `abs(x)`, `min(a, b)`, `max(a, b)` | on `int` or `fixed`. For `fixed`, the result has the larger scale of the operands. `abs` of the smallest `int` raises `Overflow`. |
| `gcd(a, b)` | the greatest common divisor of two `int` values, never negative. `gcd(0, 0)` is 0. |
| `power(b, e)` | `b` to the power `e`. `e` must not be negative (`NegativeSize`). For an `int` base the result is an `int`, with `Overflow` as usual. For a `fixed` base, `e` must be a constant, and the result is exact, with scale equal to the scale of `b` times `e`: `power(1v10, 2)` is `1v2100`. `power(b, 0)` is the `int` 1. |
| `floor(x)`, `ceiling(x)`, `truncate(x)` | the `int` found by rounding `x` toward minus infinity, toward plus infinity, or toward zero. `x` is an `int` or a `fixed`. `Overflow` if the result does not fit. |
| `sign(x)` | -1, 0 or 1 by the sign of the `int` or `fixed` `x` |
| `units(x)` | the number of smallest units in the `fixed` `x`, as an `int`: `units(12v50)` is 1250. `Overflow` if it does not fit. |
| `fromUnits(n, k)` | the `fixed` of scale `k` (a constant) with the value `n` divided by 10 to the power `k`: `fromUnits(1250, 2)` is `12v50` |
| `divide(a, b, k)`, `divide(a, b, k, mode)` | the quotient as a `fixed` of scale `k` (section 5.4) |
| `rescale(x, k)`, `rescale(x, k, mode)` | `x` as a `fixed` of scale `k` (section 5.4) |
| `RoundHalfAway`, `RoundHalfEven`, `RoundDown`, `RoundUp`, `RoundFloor`, `RoundCeiling` | the rounding modes |
| `scaleOf(x)` | the scale of the `fixed` expression `x`, as a constant `int` |
| `band(a, b)`, `bor(a, b)`, `bxor(a, b)`, `bnot(a)` | bit operations on the 64-bit two's-complement form of `int` values. They never raise `Overflow`. |
| `wrapAdd(a, b)`, `wrapSub(a, b)`, `wrapMul(a, b)` | the low 64 bits of the exact sum, difference or product, as a two's-complement `int`. They never raise `Overflow` |
| `rotl(a, n)`, `rotr(a, n)` | rotate the 64 bits of `a` left or right by `n` places. `n` must not be negative (`NegativeSize`); the count is taken modulo 64 |
| `shl(a, n)`, `shr(a, n)` | shift left, and shift right keeping the sign. `n` must not be negative (`NegativeSize`). Bits shifted out are lost. A shift by 64 or more gives 0 for `shl`, and 0 or -1 (by the sign) for `shr`. |

### 14.4 Time and random numbers

| Name | Meaning |
|---|---|
| `now()` | the time as an `int`: milliseconds since 1970-01-01 00:00 UTC. It is read once at the start of each round, so every task sees the same value during a round. |
| `sleep(ms)` | waits until the first round that starts at or after `now() + ms`. A negative `ms` raises `NegativeSize`. `sleep(0)` is the same as `sync()`. |
| `round()` | the number of the current round, a `Round` (`fixed(36, 0)`). The first round is 1, and in a `share` initializer the value is 0. It is the same for all tasks in a round and needs no recording. |
| `sleepRounds(n)` | waits until round `round() + n` begins. A negative `n` raises `NegativeSize`. It does not depend on the wall clock, so with `--strict-steps` it gives ticks. |
| `random(n)` | an `int` `r` with `0 <= r < n`. `n` less than 1 raises `NegativeSize`. |

- Each task has its own sequence of random numbers. It depends on the seed (`--seed`) and the task number. The same seed and the same task numbers give the same numbers.
- The round does not wait for time to pass, unless every task is waiting and a sleeping task is the next thing to happen. Then the program waits until the earliest wake-up time.

### 14.5 Tasks

| Name | Meaning |
|---|---|
| `spawn(body, args...)` | starts a task and gives its handle, a `task of R` or a `task` |
| `send(mb, m)` | sends a message to a mailbox or to a `task of R` |
| `receive(q, m)` | takes a message if there is one; gives `true` or `false` |
| `await(q, m)` | waits for a message |
| `sync()` | stops for the rest of the round |
| `self(q)` | a mailbox for the task's own inbox |
| `replyTo(q)`, `replyTo(q, n)`, `answer(r, m)` | makes a reply for the task's own inbox that takes one answer, or up to `n`; answers through a reply |
| `link(h)`, `cancel(h)`, `awaitEnd(h)`, `ended(h)`, `failed(h)`, `closing(h)`, `disown(h)` | calls on the handle of a child task (section 11.7) |
| `newView(pub, v)`, `publish(pub, root)`, `latest(v)`, `version(v)`, `awaitChange(v, seen)`, `ended(v)` | published views (section 11.9) |
| `Version` | the type of version numbers, `fixed(36, 0)` |
| `Round` | the type of round numbers, `fixed(36, 0)` |

### 14.6 Built-in procedures that accept several types

`len`, `fixedStr`, `strFixed`, `abs`, `min`, `max`, `scaleOf`, `power`, `floor`, `ceiling`, `truncate`, `sign`, `units`, `fromUnits`, `divide`, `rescale` and the task, handle and view procedures accept different types, and the compiler checks their uses. All other procedures have fixed types.

## 15. Compiler options

The options are given when a program is compiled. A program always runs with the values it was compiled with.

| Option | Meaning | Default |
|---|---|---|
| `--steps` | step budget `N` per task per round | 1,000,000 |
| `--strict-steps` | a task over its budget raises `OutOfSteps` and does not yield | off |
| `--memory` | memory limit per task, in size units | 16 Mi (16,777,216) |
| `--depth` | deepest allowed nesting of calls | 10,000 |
| `--init-steps` | step limit for each shared-value initializer | 100,000,000 |
| `--init-memory` | memory limit for each initializer, in size units | 64 Mi |
| `--shared-memory` | size limit of the shared area, in size units | 256 Mi |
| `--grace` | rounds that a closing task may run before it is stopped | 100 |
| `--grace-time` | milliseconds that a closing task may spend waiting before its waiting calls raise `Timeout` | 5000 |
| `--socket-buffer` | bytes buffered for each socket half before a write waits | 65,536 |
| `--handles` | open files and sockets per task | 1024 |
| `--seed` | seed for `random` | chosen by the operating system and recorded |
| `--path` | directories searched for modules, in order | the directory of the main module |

## 16. What is the same on every run

Given the same compiler options, the same arguments, and the same outside input (the contents of files read, the lines of standard input, the clock values and the seed), taking effect in the same rounds, a program gives the same messages, output and result every time, whatever the number of processors. The round in which an outside event takes effect depends on timing. Recording each outside event together with its round is enough to repeat a run exactly.

- No changeable memory is reachable from two tasks.
- Closing down follows a fixed order, and each wave of tasks that close together is decided by the state at a round boundary.
- A version of a view becomes visible at a round boundary, and when two tasks answer one reply in the same round, the lower task number wins.
- No operation shows the address or identity number of an object.
- There are no finalizers, no weak references, and no way to ask for the size of the heap.
- Library structures that list their contents do so in the order they were added.

## Appendix A. Grammar

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

## Appendix B. Glyphs

| Glyph | Meaning |
|---|---|
| `:=` | assignment |
| `:` | "has type" |
| `is` | "is defined as" (constants, types, shared values) |
| `=` | value equality |
| `<` `<=` `>` `>=` | ordering |
| `~` | same object |
| `+` `-` `*` | add, subtract, multiply |
| `_` | minus sign in front of a number |
| `/` | decimal division |
| `div` `mod` | whole-number division and remainder |
| `.` | field |
| `::` | module name qualifier |
| `v` | decimal point in a decimal literal |
| `\` | starts an escape in a character or string literal |
| `#` | starts a comment |

## Appendix C. Not in this version

Ready-made collections (lists, maps, sets) in the library, character classification and case conversion, running other programs and reading environment variables, seeking in files, checking for input without waiting, handling signals in a program, lossy notifications between tasks, TLS and datagrams for sockets, and waiting on several sources at once.
