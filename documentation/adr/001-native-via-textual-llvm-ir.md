# ADR-001: Native code through *textual* LLVM IR

Status: accepted. Plan reference: D1.

## Context

Alúma must produce native code for x86-64 Linux. Three routes were available: link
`libLLVM` through `llvm-sys`/`inkwell`, emit textual `.ll` and drive `clang` as a
subprocess, or emit machine code directly.

## Decision

`almbe` emits **textual LLVM IR** and drives `clang` and the system linker as
subprocesses. Target: x86-64 Linux, System V ABI, only.

## Reasons

- **It keeps the door to Alúma open.** Linking `libLLVM` makes `almbe` unportable to
  Alúma forever: the language has no foreign-function mechanism at all — nothing in
  [1]–[9] can call a C library, and none is deferred, so this is permanent rather than
  pending. A text emitter, by contrast, is a program that writes a text file, which
  Alúma *can* do. The only thing then blocking the port after M9 is running `clang`,
  and running other programs is merely **deferred** ([12] "Deferred", item 3: shell
  interop), with a shell driver covering it in the meantime (plan §0).
- **Every miscompile starts as a readable diff.** With no interpreter (ADR-013) a
  codegen bug surfaces only as a wrong answer, so inspectable output is not a
  convenience, it is the debugging method.
- **It tolerates LLVM version drift.** Linking pins a version and makes the build
  painful; text is stable at the edges, and CI can test against two versions.

## Consequences

- A process spawn and a textual round trip per compile. This costs compile time only;
  revisit if `almc` compiling itself becomes dominated by it.
- Emission must stay inside a conservative subset of the IR language, since textual
  `.ll` is where version drift bites. Pinned in CI, tested against two versions.
- `clang` is a build-time dependency of the toolchain, not of the compiler's own build.
- Targets beyond x86-64 Linux are out of scope until after M9.
