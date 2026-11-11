# Extracted documentation tests

One test directory per code example in `documentation/language_reference.md`,
`guide.md` and `style_guide.md`, extracted mechanically with the outcome the document
states. [11] is effectively an acceptance suite, so this is the first population of the
conformance corpus (ADR-013.4).

The extractor lives in `tools/`; its output is checked in so that a test can be read and
run without running the extractor, and so that a change in the docs shows up as a diff
here.
