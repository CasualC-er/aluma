#!/usr/bin/env python3
"""Extract the code examples from the documentation into conformance tests.

Plan reference: Phase 0, D13.4. [11] is effectively an acceptance suite, so the first
population of the corpus is mechanical rather than hand-written.

Run from the repository root:

    python3 -I tools/extract_doc_tests.py

It writes one test directory per extracted example under tests/docs/, and an
INVENTORY.md recording **every** fenced block in the three documents together with what
became of it. The inventory is the point: a block that cannot be turned into a test must
be visible, with a reason, or the corpus silently under-covers the spec and nothing says
so. Nothing is deleted that the extractor did not write.

Classification, in order:

  tagged `ebnf` / `keywords`   reference material, not a program            -> skipped
  tagged `text`                terminal output or a transcript              -> skipped
  tagged `fragment`            not a compilable unit by the doc's own word  -> skipped
  declares `start`             a program [9.1]                              -> run test
  holds declarations           a declaration set, nothing to execute        -> compile-ok
  anything else                prose, output, or a snippet                  -> skipped

A `run` test needs its expected output, which the documents state in prose as often as
in a block. The extractor pins an expectation only when the example is self-evidently
total: a following fenced block that is plainly output. Otherwise it writes
`[stdout] ignore` and records the test as UNPINNED in the inventory, to be completed by
hand. An unpinned test still has value — it must compile, link and exit 0 — but it is
not a conformance test until its output is pinned, and the inventory is what stops that
being forgotten.
"""

from __future__ import annotations

import os
import re
import shutil
import sys
from dataclasses import dataclass, field

DOCS = [
    "documentation/language_reference.md",
    "documentation/guide.md",
    "documentation/style_guide.md",
]
OUT = "tests/docs"
MARKER = ".extracted"  # written into every generated directory

# A module name is an identifier; a declaration line starts with one of these.
DECL = re.compile(r"^\s*(?:also\s+|private\s+)?(proc|type|const|enum|exception|share|generic|import)\b")
START = re.compile(r"^\s*(?:proc\s+)?start\b", re.M)
# `# Stack.alm` or `# module Stack` as the first line of a block names its module.
MODULE_HINT = re.compile(r"^#\s*(?:module\s+)?([A-Za-z][A-Za-z0-9]*)(?:\.alm)?\s*$")
OUTPUT_ISH = re.compile(r"^(uncaught|failed|[A-Za-z0-9].*)$")


@dataclass
class Block:
    doc: str
    line: int          # 1-based line of the opening fence
    tag: str
    body: str
    heading: str
    lead: str = ""     # the prose line just before the fence
    # what became of it
    disposition: str = "pending"
    reason: str = ""
    test: str = ""
    extras: list[str] = field(default_factory=list)


def read_blocks(path: str) -> list[Block]:
    lines = open(path, encoding="utf-8").read().split("\n")
    out: list[Block] = []
    heading = ""
    lead = ""
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("#"):
            heading = line.lstrip("#").strip()
        elif line.strip() and not line.startswith("```"):
            lead = line.strip()
        m = re.match(r"^```(\w*)\s*$", line)
        if not m:
            i += 1
            continue
        tag = m.group(1) or ""
        j = i + 1
        body: list[str] = []
        while j < len(lines) and not lines[j].startswith("```"):
            body.append(lines[j])
            j += 1
        out.append(
            Block(
                doc=path,
                line=i + 1,
                tag=tag,
                body="\n".join(body),
                heading=heading,
                lead=lead,
            )
        )
        i = j + 1
    return out


# "Module `IntList`:" — how all three documents introduce a module's source.
DEFINES_MODULE = re.compile(r"^Module\s+`([A-Za-z][A-Za-z0-9]*)`")
# `import A;`, `import A, B;`, `import X is G(...);` — the file needed is A, B, or G.
IMPORT_LINE = re.compile(r"^\s*import\s+([^;]+);")
# Library modules that need no file: `std` is automatic [4.1] and `Net` is built in [9.7].
BUILTIN_MODULES = {"std", "Net"}


def defined_module(block: Block) -> str | None:
    """The module this block is the source of, if the document says so."""
    m = DEFINES_MODULE.match(block.lead)
    return m.group(1) if m else None


def imports_needed(body: str) -> list[str]:
    """The module files a body needs, in order of first appearance."""
    needed: list[str] = []
    for line in body.split("\n"):
        m = IMPORT_LINE.match(line)
        if not m:
            continue
        for item in m.group(1).split(","):
            item = item.strip()
            # `X is G(args)` imports an instance of the generic module G.
            inst = re.match(r"[A-Za-z][A-Za-z0-9]*\s+is\s+([A-Za-z][A-Za-z0-9]*)", item)
            name = inst.group(1) if inst else item.split()[0] if item.split() else ""
            if name and name not in BUILTIN_MODULES and name not in needed:
                needed.append(name)
    return needed


def slug(text: str, fallback: str) -> str:
    s = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return (s or fallback)[:48]


def module_name(block: Block) -> tuple[str, str]:
    """Return (module name, body). The document's own name wins, because an `import`
    elsewhere refers to it and a module is its file name [4.1]."""
    named = defined_module(block)
    if named:
        return named, block.body
    lines = block.body.split("\n")
    if lines:
        m = MODULE_HINT.match(lines[0].strip())
        if m:
            return m.group(1), "\n".join(lines[1:]).lstrip("\n")
    name = re.sub(r"[^A-Za-z0-9]", "", block.heading.title()) or "Example"
    return name[:24], block.body


def is_output_block(block: Block) -> bool:
    if block.tag == "text":
        return True
    if block.tag or DECL.search(block.body):
        return False
    # No statement terminators and no keywords: prose or program output.
    return ";" not in block.body


# Blocks that look like programs but are not, listed by (document, line) with the
# reason. A mechanical classifier cannot tell a schema from a program: both are
# declarations, and only a reader knows that `params`, `result` and `block` in
# "proc name(params): result block;" are placeholders standing for the shape of a
# declaration. One hand-listed exception with a stated reason is more honest than a
# heuristic that guesses, and the inventory shows it as skipped either way.
SCHEMATIC = {
    ("documentation/language_reference.md", 114): (
        "a schema of the declaration forms, not a program: `params`, `result` and "
        "`block` are placeholders"
    ),
}


def classify(block: Block) -> None:
    schematic = SCHEMATIC.get((block.doc, block.line))
    if schematic:
        block.disposition, block.reason = "skipped", schematic
        return
    if block.tag in ("ebnf", "keywords"):
        block.disposition, block.reason = "skipped", f"`{block.tag}` is reference material"
    elif block.tag == "text":
        block.disposition, block.reason = "skipped", "terminal output or transcript"
    elif block.tag == "fragment":
        block.disposition, block.reason = "skipped", "marked `fragment`: not a compilable unit"
    elif START.search(block.body):
        block.disposition = "run"
    elif DECL.search(block.body):
        block.disposition = "compile-ok"
    else:
        block.disposition, block.reason = "skipped", "no declaration and no `start`"


def write_test(
    block: Block,
    index: int,
    expected: Block | None,
    registry: dict[str, Block],
) -> None:
    doc = os.path.basename(block.doc).removesuffix(".md")
    name = f"{index:03d}-{slug(block.heading, 'example')}"
    d = os.path.join(OUT, doc, name)
    os.makedirs(d, exist_ok=True)
    open(os.path.join(d, MARKER), "w").close()

    mod, body = module_name(block)
    with open(os.path.join(d, f"{mod}.alm"), "w", encoding="utf-8") as f:
        f.write(body.rstrip("\n") + "\n")

    # A module this example imports must sit beside it, or the compile fails for a
    # reason that has nothing to do with the rule the test exists to pin. Transitive,
    # because a pulled-in module may import another.
    pending = imports_needed(body)
    written = {mod}
    unresolved: list[str] = []
    while pending:
        need = pending.pop(0)
        if need in written:
            continue
        source = registry.get(need)
        if source is None:
            unresolved.append(need)
            continue
        with open(os.path.join(d, f"{need}.alm"), "w", encoding="utf-8") as f:
            f.write(source.body.rstrip("\n") + "\n")
        written.add(need)
        block.extras.append(need)
        pending.extend(imports_needed(source.body))
    if unresolved:
        block.extras.append("MISSING:" + "+".join(sorted(unresolved)))

    lines = [
        f"# name: {block.heading or name}",
        f"# source: {block.doc}:{block.line}",
        "# extracted by tools/extract_doc_tests.py — edit freely, it will not be overwritten",
        f"mode = {block.disposition}",
        f"main = {mod}.alm",
    ]
    if block.disposition == "run":
        lines.append("exit = 0")
        if expected is not None:
            lines += ["", "[stdout]", expected.body.rstrip("\n")]
            block.reason = f"output from the block at line {expected.line}"
        else:
            lines += ["", "[stdout] ignore"]
            block.disposition = "run (UNPINNED)"
            block.reason = "the document states no output block; pin it by hand"
    with open(os.path.join(d, "test.manifest"), "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    block.test = os.path.join(doc, name)


def clean_generated() -> int:
    """Remove only directories this script wrote, so hand-written tests survive."""
    removed = 0
    for root, dirs, files in os.walk(OUT):
        if MARKER in files:
            shutil.rmtree(root)
            removed += 1
            dirs[:] = []
    return removed


def main() -> int:
    if not os.path.isdir("documentation"):
        print("run me from the repository root", file=sys.stderr)
        return 2
    removed = clean_generated()

    blocks: list[Block] = []
    for d in DOCS:
        blocks.extend(read_blocks(d))
    for b in blocks:
        classify(b)

    # Which block is the source of which module, as the documents declare it.
    registry: dict[str, Block] = {}
    for b in blocks:
        name = defined_module(b)
        if name and b.disposition in ("run", "compile-ok"):
            registry.setdefault(name, b)

    index = 0
    for i, b in enumerate(blocks):
        if b.disposition not in ("run", "compile-ok"):
            continue
        index += 1
        expected = None
        if b.disposition == "run":
            nxt = blocks[i + 1] if i + 1 < len(blocks) else None
            if nxt and nxt.doc == b.doc and is_output_block(nxt):
                expected = nxt
                nxt.disposition = "skipped"
                nxt.reason = f"expected output of {b.heading or b.line}"
        write_test(b, index, expected, registry)
        if any(e.startswith("MISSING:") for e in b.extras):
            missing = next(e for e in b.extras if e.startswith("MISSING:"))[len("MISSING:"):]
            b.disposition += " (NEEDS MODULES)"
            b.reason = f"imports {missing}, which no block in the documents defines"
        elif b.extras:
            b.reason = (b.reason + "; " if b.reason else "") + "with " + ", ".join(
                e for e in b.extras if not e.startswith("MISSING:")
            )

    counts: dict[str, int] = {}
    for b in blocks:
        counts[b.disposition] = counts.get(b.disposition, 0) + 1

    with open(os.path.join(OUT, "INVENTORY.md"), "w", encoding="utf-8") as f:
        f.write("# Extraction inventory\n\n")
        f.write(
            "Generated by `tools/extract_doc_tests.py`. Every fenced block in the three\n"
            "documents appears here with what became of it, so that a block the extractor\n"
            "could not use is visible rather than silently missing from the corpus\n"
            "(ADR-013.4).\n\n"
            "`run (UNPINNED)` means the example is a program whose output the document\n"
            "states in prose rather than in a block: the test compiles, links and expects\n"
            "exit 0, but its output is not pinned yet. Those are the ones to finish by hand.\n\n"
            "`NEEDS MODULES` means the example imports a module that no block in the three\n"
            "documents defines, so it cannot compile as extracted; the module has to be\n"
            "written by hand before the test means anything.\n\n"
        )
        for k in sorted(counts):
            f.write(f"- **{k}**: {counts[k]}\n")
        f.write(f"\nTotal blocks: {len(blocks)}\n\n")
        f.write("| Document | Line | Tag | Heading | Disposition | Test / reason |\n")
        f.write("|---|---|---|---|---|---|\n")
        for b in blocks:
            where = b.test or b.reason
            heading = (b.heading or "").replace("|", "\\|")
            f.write(
                f"| {os.path.basename(b.doc)} | {b.line} | {b.tag or '—'} | {heading} "
                f"| {b.disposition} | {where} |\n"
            )

    print(f"removed {removed} previously generated directories")
    for k in sorted(counts):
        print(f"  {k}: {counts[k]}")
    print(f"wrote {index} tests and {OUT}/INVENTORY.md")
    return 0


if __name__ == "__main__":
    sys.exit(main())
