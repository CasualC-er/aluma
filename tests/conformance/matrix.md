# Conformance matrix

Every numbered rule of `documentation/language_reference.md` and the test or tests that
pin it. The plan makes this a Phase 0 deliverable and a Phase 8 CI gate (ADR-013.4),
for one reason: with no interpreter, a codegen or semantic bug surfaces only as a wrong
answer, so coverage *is* the safety net. A rule with no test is not implemented, however
much code appears to implement it.

**How to use it.** Implementing a rule means adding its row here with the test that
pins it. A row with no test is a rule the compiler may get wrong silently. The matrix
is checked in CI from Phase 8; until then it is checked by hand at the end of each
phase.

Status values: `tested` (a test pins it), `partial` (some cases pinned), `todo`.

A row naming a directory under `tests/conformance/` is a test the compiler binary is
actually run against, end to end, by `src/almc0/tests/conformance.rs`. A row naming a
Rust test is a unit test of a component. Both count, but only the first kind proves the
diagnostic a user sees.

Where a test lives:

| Place | Holds |
|---|---|
| `tests/docs/` | extracted from the documents; see `INVENTORY.md` |
| `tests/conformance/` | hand-written, one per rule the documents do not reach |
| `src/*/src/**.rs` `#[cfg(test)]` | unit tests of a component against the spec text |
| `src/almc0/tests/` | whole-corpus integration tests |

---

## The errors the specification names explicitly

These are the sentences in the reference that say "is a compile error" or "is an error".
They are the floor, not the whole list: most rules are stated as "must", "cannot" or
"only", and each of those is an error too. The Phase 2 and Phase 8 sections of `TODO.md`
enumerate those, and rows are added here as each is implemented.

| § | Rule | Test | Status |
|---|---|---|---|
| [2] | a literal larger than the largest `int`, except `9223372036854775808` / `0x8000000000000000` under `_` | `lex.rs::the_over_max_rule_and_its_two_spellings`, `parse.rs::the_over_max_literal_is_legal_only_under_underscore`, `tests/conformance/lex-over-max-literal` | tested |
| [2] | any escape outside `\n \t \r \0 \\ \' \" \u{h}` | `lex.rs::the_escape_set_is_exactly_the_one_in_the_spec`, `tests/conformance/lex-escape-set` | tested |
| [2] | a literal cannot contain a raw line break | `lex.rs::a_literal_cannot_hold_a_raw_line_break` | tested |
| [2] | a char literal denotes exactly one code point | `lex.rs::a_char_literal_is_exactly_one_code_point` | tested |
| [2] | a `fixed` literal is decimal (amendment A-001) | `lex.rs::a_fixed_literal_is_decimal_only` | tested |
| [4.1] | `import` lines come before the other declarations | `parse.rs::generic_lines_come_first_and_imports_next` | partial |
| [4.1] | using a library name before the declaration that hides it | — | todo |
| [4.1] | `private` on anything other than the declarations it may prefix | — | todo |
| [4.3] | a type group cannot contain a cycle of containment by value; an alias loop | — | todo |
| [4.4] | instantiations nest at most 32 deep | — | todo |
| [5] | calling a function as a plain statement; `void` on a procedure with no result | — | todo |
| [7.4] | `/` with two `int` operands | — | todo |
| [7.4] | two different tags in `+`, `-` or a comparison | — | todo |
| [10] | relations do not chain | `parse.rs::relations_do_not_chain`, `tests/conformance/parse-relations-do-not-chain` | tested |
| [10] | every other shape of syntax error | `parse.rs::a_negative_test_for_each_shape_of_syntax_error` | partial |

## Lexical structure [2]

| Rule | Test | Status |
|---|---|---|
| identifiers are ASCII, case-sensitive, no underscores | `lex.rs::identifiers_are_ascii_and_hold_no_underscore` | tested |
| the 52 reserved words | `token.rs::the_keyword_table_is_the_one_in_the_specification` | tested |
| `#` to end of line, not inside a literal | `lex.rs::comments_run_to_the_end_of_the_line` | tested |
| integer literals, decimal and `0x` | `lex.rs::integer_literals_decimal_and_hex` | tested |
| `fixed` literal scale is the digits after `v` | `lex.rs::fixed_literals_keep_their_digits_and_scale` | tested |
| `\u{h}`: one to six digits, a valid code point | `lex.rs::unicode_escapes` | tested |
| the glyph table, and the absence of a not-equal glyph | `token.rs::the_glyph_table_holds_what_the_specification_lists`, `lex.rs::there_is_no_not_equal_glyph` | tested |
| char literals: the rules have no cover in the extracted corpus | `lex.rs` unit tests only — see README | partial |

## Grammar [10]

| Rule | Test | Status |
|---|---|---|
| every declaration form | `parse.rs::the_declaration_forms_of_section_four` | tested |
| `also` and `private` prefixes | `parse.rs::also_and_private_prefix_a_declaration` | tested |
| every type form | `parse.rs::every_type_of_section_three` | tested |
| every statement form | `parse.rs::every_statement_of_section_five` | tested |
| precedence levels 1–8 | `parse.rs::precedence_follows_the_levels_of_section_six` | tested |
| `not` binds looser than the comparisons | `parse.rs::not_binds_looser_than_a_comparison` | tested |
| selectors on a call result | `parse.rs::selectors_are_allowed_on_a_call_result` | tested |
| `garg` is a type or an expression | `parse.rs::a_generic_argument_may_be_a_type_or_an_expression` | tested |
| the whole grammar, over the documents' examples | `src/almc0/tests/parse_corpus.rs` | tested |

## Modules and visibility [4.1] — Phase 2

| Rule | Test | Status |
|---|---|---|
| a module is its file name without the extension, an identifier that is not a keyword | `load.rs::what_may_be_a_module_name` | tested |
| `A` is `A.alm`, searched on `--path`, first match wins | `load.rs::the_first_directory_on_the_path_wins` | tested |
| `--path` defaults to the main module's directory [8.9] | `load.rs::the_path_defaults_to_the_main_modules_directory` | tested |
| the implementation's modules are searched after the path | `load.rs::an_implementation_module_needs_no_file` | tested |
| a program's own `Net.alm` takes the implementation's place | `load.rs::a_programs_own_net_takes_the_implementations_place` | tested |
| the program is the main module and everything it imports, transitively | `load.rs::a_module_imports_come_before_it`, `load.rs::a_diamond_loads_the_shared_module_once` | tested |
| an instance import loads the generic module [4.4] | `load.rs::an_instance_import_loads_the_generic_module` | tested |
| a module cannot import itself | `load.rs::a_module_cannot_import_itself`, `tests/conformance/load-self-import` | tested |
| the imports must not form a cycle, and the error names the cycle | `load.rs::a_cycle_is_reported_naming_the_cycle`, `tests/conformance/load-import-cycle` | tested |
| a module that is not on the path | `load.rs::a_missing_module_names_no_directory`, `tests/conformance/load-missing-module` | tested |
| only the main module may declare `start` | `load.rs::only_the_main_module_may_declare_start`, `tests/conformance/load-start-outside-main` | tested |
| the main module must declare `start`, in one of the four forms [9.1] | — | todo |
| imports are not transitive | `resolve.rs::imports_are_not_transitive`, `tests/conformance/resolve-imports-not-transitive` | tested |
| one flat namespace per module | `resolve.rs::a_module_has_one_namespace` | tested |
| an imported module's name occupies the namespace | `resolve.rs::an_imported_module_name_occupies_the_namespace` | tested |
| two modules may declare the same name | `resolve.rs::two_modules_may_declare_the_same_name` | tested |
| `std` is imported automatically, unqualified or qualified | `resolve.rs::the_library_is_in_scope_unqualified_and_qualified` | tested |
| a top-level declaration hides a library name; `std::name` still reaches it | `resolve.rs::a_declaration_hides_a_library_name_but_std_still_reaches_it` | tested |
| a use before the hiding declaration is an error, not the library name | `resolve.rs::using_a_hidden_library_name_before_its_declaration_is_an_error`, `tests/conformance/resolve-use-before-declaration` | tested |
| the unhidable names, for declarations, parameters and locals | `resolve.rs::an_unhidable_library_name_cannot_be_declared_or_shadowed`, `library.rs::the_unhidable_list_is_the_one_in_the_spec`, `tests/conformance/resolve-unhidable-library-name` | tested |
| `std` cannot be declared [2] | `resolve.rs::std_cannot_be_declared` | tested |
| `private` on a declaration, across modules | `resolve.rs::a_private_name_cannot_be_used_from_another_module`, `tests/conformance/resolve-private-across-modules` | tested |
| `private` record fields, and private types in visible signatures | — | todo (needs the type graph) |
| qualification has one level only: no `A::B::c` | the grammar admits only one level; `parse.rs` | tested |
| declaration before use, and `also` groups [4.3] | `resolve.rs::a_group_lets_its_members_name_each_other`, `resolve.rs::without_also_a_forward_reference_is_an_error`, `resolve.rs::also_cannot_come_first_or_follow_the_wrong_kind`, `tests/conformance/resolve-also-group-rules` | tested |
| a name must denote the right kind of thing | `resolve.rs::a_name_must_denote_the_right_kind_of_thing` | tested |
| the library's names and kinds [9] | `library.rs` tests | tested |

## Other semantics [4.2]–[4.4], [7] — Phase 2

Rows are added as each rule is implemented, with its negative test. The enumeration to
work through is the Phase 2 checklist in `TODO.md`, which was derived from these
sections; nothing there may be ticked without a row here.

## Runtime [7.5], [8], [9] — Phase 4 onward

| Rule | Test | Status |
|---|---|---|
| each of the 22 predefined exceptions is raised where the table says | — | todo |
| the `uncaught` and `failed` report forms | — | todo |
| exit statuses 0, the `int` form, 1, 130, 143 | — | todo |
