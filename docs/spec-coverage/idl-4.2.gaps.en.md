# OMG IDL 4.2 — negative list

**Spec:** [OMG IDL 4.2](https://www.omg.org/spec/IDL/4.2/), formal/18-01-05,
132 pages. PDF in the local cache at
`internal/standards/cache/omg/idl-4.2.pdf` (gitignored).

**Tool:** `tools/idl-4.2-gap-scan/scan.sh`. **Evidence:**
`tools/idl-4.2-gap-scan/cases.tsv` (357 probes) and
`tools/idl-4.2-gap-scan/expected.tsv` (the recorded state).

The other pages under `docs/spec-coverage/` are positive lists: they name
what was looked at and how far it got. What was never looked at cannot
appear in them, because their content comes from the tree and not from
the specification. This page is the other direction. Its denominator is
Annex A of the specification — all 227 grammar productions — plus the 24
standardized annotations of clause 8 and the normative clauses of 7.2
(lexis), 7.3 (preprocessing), 7.4.13 and 7.4.15 (semantics of bit sets,
bit masks and annotation application) and 7.5 (names and scoping).

Every row here has an attempt behind it: the IDL that was fed in and what
came back. No row rests on a guess.

## How it was measured

One minimal `.idl` file per construct, through all six subcommands of
`zerodds-idlc`. A construct can be missing from any one of them
independently, and the first version of this survey measured only `check`
and `generate` — the other four stood there as an assumption. That is the
same kind of positive list this page was written against, so the
measurement is now a matrix of probe × subcommand:

| Column | Command | Question |
|---|---|---|
| `check` | `zerodds-idlc check` | does the front end recognize the construct |
| `strict` | `zerodds-idlc check --strict` | does `--strict` change that |
| `lint` | `zerodds-idlc lint` | does it pass the rules a valid file can still break |
| `format` | `zerodds-idlc format` | is there a canonical form |
| `trip` | `format`, reparse, compare | does the round trip hold |
| `dump` | `zerodds-idlc dump-ast` | does the model show the construct |
| `deps` | `zerodds-idlc print-deps` | does the dependency walk carry it |
| `rust` | `zerodds-idlc generate --lang rust` | is there an emission |
| `c` | `zerodds-idlc generate --lang c` | the same in the second backend |

`trip` is the column against a silent loss. A construct the parser knows
and the formatter does not disappears when a file is reformatted: no
error, just a file with something missing from it. Comparing the two
formatted texts does not catch that — the second `format` of a text that
already lost the construct reproduces the loss. So the comparison is over
the **model**: `dump-ast` and `print-deps` of the source against
`dump-ast` and `print-deps` of the formatted output, plus idempotence of
the format itself. `-` stands where there is nothing to compare, because
`format` or `check` refused the file.

Plus one measurement that does not depend on the tool: `fixtures` counts
how many of the 748 committed `*.idl` files in the tree contain the
construct at all, by a text pattern (column `witness` in `cases.tsv`).
That is weaker than "there is a test for it" — a zero is nevertheless a
fact: no IDL in the tree carries the construct, so nothing in the tree
can have exercised it. A `-` means no text pattern isolates the
construct; there the fixture question is not measurable this way and
stands as unchecked.

## The number

357 probes. The findings fall out like this — F overlaps the other
categories, because the same probe can have one finding in `check` and a
different one in `format`:

| | Category | Probes |
|---|---|---|
| A | not recognized — the front end rejects it | 14 → 13 → **0** |
| B | accepted although the specification requires a rejection | 19 → **2** |
| C | recognized, not emitted | 12 → 75 → **74** |
| D | emitted, but no `.idl` in the tree contains it | 33 → 26 → **0** |
| E | `--strict` has no effect | 4 → **0** |
| F | `format` accepts what `check` refuses | 21 → **0** |
| G | `format` refuses what `check` accepts | 3 → **0** |

A is empty: the three grammar forms of A.1 to A.3 and the eight
preprocessor forms of A.5 pass `check`. C grew by 63 probes without
anything being overlooked: the C column refuses every `interface` since
`7b651ac2643` made an interface binding refuse what it cannot place in
full, and the Rust column refuses the object type a port or connector
carries (§C below); the union with no `default:` is out of C (§C). G
came from A.5 and is empty: `format` checks the file as `check` does,
after the preprocessor (§G).

At production level, counted over the probes of category `grammar` in
`cases.tsv` (161 productions carry a probe): 73 have a finding — none a
rejected form, 64 a form with no emission, 14 a form with no fixture
(with overlaps). Before the re-recording the same count gave 27 (3, 12,
14). The figures of earlier versions of this page (227 productions, 37
with a finding) were bounded differently and do not compare with these.

Category B is none of the four categories the audit asked for. It had to
be added, because the front end can miss not only what the specification
allows but also what it forbids — and in the second case the result is
not an error message but a model with wrong content. Three of the 19 rows
are exactly that: the declared type of a constant disappears and is
replaced by the type of the literal.

## The matrix

Where each subcommand stands over all 357 probes. Per probe it is in
`tools/idl-4.2-gap-scan/expected.tsv`, one row per probe and one column
per subcommand — the evidence this page cites.

| Column | `ok` | `err` | `-` |
|---|---|---|---|
| `check` | 324 | 33 | — |
| `strict` | 321 | 36 | — |
| `lint` | 324 | 33 | — |
| `format` | 321 | 36 | — |
| `trip` | 321 | — | 36 |
| `dump` | 324 | 33 | — |
| `deps` | 324 | 33 | — |
| `rust` | 307 | 50 | — |
| `c` | 249 | 108 | — |

`check`, `lint`, `dump` and `deps` agree probe for probe: all four go
through `analyse()`. `format` stands three probes below, which is G.
`strict` stands three probes below — the three vendor directives of E.
`trip` is `-` wherever `check` or `format` refused the file and `ok`
everywhere else: no silent loss in the formatter.

`rust` and `c` stand below, and that is category C: 50 and 108 probes
respectively that the front end recognizes and a backend deliberately
does not emit.

The fixture counts in the tables of this page are from before the
re-recording, counted over 748 files; `expected.tsv` carries them over
the 821 files the tree has now.

---

## A — not recognized

The specification allows it; `zerodds-idlc check` rejects it.

### A.1 `(17) <literal>` — adjacent string literals (closed)

**Now:** `ok` in all nine columns (`5cd950c38f2`). Evidence:
`crates/idl-lower/tests/idl_4_2_grammar.rs`. The finding as it stood:

**Spec:** §7.2.6.3 — "Adjacent string literals are concatenated.
Characters in concatenated strings are kept distinct."

**IDL:** `const string K = "ab" "cd";`

**Result:** `error: [IDL0002] expected ';' after the constant` (1:23),
followed by `[IDL0001] expected a declaration`.

**Fixtures:** 0 of 748.

**Probe:** `bb1-017-lit-string-concat`

### A.2 `(64) <type_declarator>` — `<constr_type_dcl>` inside a `typedef` (closed)

**Now:** `ok` in all nine columns (`5cd950c38f2`). Evidence:
`crates/idl-lower/tests/idl_4_2_grammar.rs`. The finding as it stood:

**Spec:** §7.4.1.4.4.7 "Naming Data Types", production 64 —
`<type_declarator> ::= { <simple_type_spec> | <template_type_spec> | <constr_type_dcl> } <any_declarators>`.
The third alternative defines a struct, union or enum inside the
`typedef` itself.

**IDL:** `typedef struct Inline { long m; } Ali;`

**Result:** `error: [IDL0001] expected a type` (1:9), three follow-on errors.

**Fixtures:** 0 of 748.

**Probe:** `bb1-064-typedef-constr`

### A.3 `(112) <export> ::+ …` — `typeid`, `typeprefix` and `import` in an interface body (closed)

**Now:** `check` to `deps` and `rust` `ok` (`5cd950c38f2`); `c` refuses
it as it refuses every `interface` (category C). Evidence:
`crates/idl-lower/tests/idl_4_2_grammar.rs`. The finding as it stood:

**Spec:** §7.4.6.3, production 112 —
`<export> ::+ <type_id_dcl> ";" | <type_prefix_dcl> ";" | <import_dcl> ";" | <op_oneway_dcl>`.

At file scope all three work (probes `bb6-113-typeid`,
`bb6-114-typeprefix`, `bb6-115-import-scoped`, all `ok`). Only the
extension of `<export>` is missing — the form inside an interface body.

**IDL / result:**

| IDL | first error |
|---|---|
| `interface Ia { typeid Ia "IDL:Ia:2.0"; };` | `[IDL0001] expected a type` (1:16) |
| `interface Ia { typeprefix Ia "ex.org"; };` | `[IDL0001] expected a type` (1:16) |
| `interface Ia { import Mb; };` | `[IDL0001] expected a type` (2:19) |

**Fixtures:** not measurable by text search.

**Probes:** `bb6-112-export-typeid`, `bb6-112-export-typeprefix`,
`bb6-112-export-import`

### A.4 §7.4.15.4.2 — unknown annotation with an identifier argument (closed)

**Spec:** §7.4.15.4.2 — "in case an unknown annotation is encountered, it
should be ignored by the IDL-processing tools."

Two of the three forms were ignored, as the clause requires. The third
was not:

| IDL | before | now |
|---|---|---|
| `@nosuchthing struct Sa { long m; };` | 0 errors | 0 errors |
| `@nosuchthing(v = 1) struct Sa { long m; };` | 0 errors | 0 errors |
| `@nosuchthing(v = SOME_NAME) struct Sa { long m; };` | `[IDL0010] a name does not resolve: … ["SOME_NAME"]` | **0 errors** |

An identifier used as an argument value was resolved even though the
annotation it belongs to is to be ignored. Every file writing
`@RTI_topic_kind(NO_KEY)` or `@allowed_data_representation(XCDR2)` failed
on it — the form vendor annotations use for their enum values. This is
not a survey case: it is the IDL users hand the tool.

**The fix.** `Lowering::annotation` in
`crates/idl-lower/src/annotation.rs` decides whether the annotation is
known — in `BUILTIN_ANNOTATIONS`, or bound in scope as an
`AnnotationDef`. If it is not, an argument whose value is an identifier
is not lowered as an expression. The mechanism already existed for the
builtins with an enumerated parameter (`@autoid(HASH)`,
`@extensibility(FINAL)`) and now carries both cases: the identifier
belongs to the annotation's own type and is in no scope of the file.

A **literal** is still lowered — `@nosuchthing(v = 1)` keeps its 1. Only
an identifier has nothing in this file to point at.

**A second defect of the same family, fixed with it.** `@verbatim` was
missing from the list of builtins with an enumerated parameter, although
IDL 4.2 §8.3.5.1 declares `PlacementKind placement`. Every
`@verbatim(placement = BEFORE_DECLARATION)` in the tree was refused over
the enumerator. `verbatim_annotation` reads all three parameters out of
`Argument::text` and `Argument::identifier` anyway, so nothing is lost.

`crates/idl-parse/tests/fixtures/spec_features/rti_connext_pragmas.idl`
carried both defects in one file and now passes `check`.

**Tests:** `tools/idlc/tests/annotation_arguments.rs` — six tests at the
compose level, because the diagnostic came from the resolve pass. The
first draft sat in `idl-lower` and would have been green with or without
the fix; a mutation showed that.
Mutation: switching the condition off (3 red), discarding literals too
(1 red), taking `verbatim` off the list (2 red), treating a declared
annotation as unknown (1 red).

**Probe:** `sem-7.4.15-unknown-annotation-enum-arg` — now `ok`.

### A.5 §7.3 — the preprocessor is not the one from ISO/IEC 14882:2003 (closed)

**Now:** all eight probes `ok` in `check`, `lint`, `dump`, `deps`, `rust`
and `c` (`f05fd8b6991`): function-like macros, `#`, `##` with
placemarkers, the rescan with hide sets, `#if` over a full constant
expression, `__LINE__`, `__FILE__`, line continuation. New code
`IDL0022`. Evidence: `crates/idl-parse/tests/preprocessor_iso_14882.rs`.
`format` refuses three of them, see G. The finding as it stood:

**Spec:** §7.3 — "IDL shall be preprocessed according to the
specification of the preprocessor in ISO/IEC 14882:2003."

Six forms of that preprocessor are missing. Two of them report
themselves with their own diagnostic — the better half of the finding,
because a silent failure here would have swallowed text.

| IDL | Result |
|---|---|
| `#define TWICE(x) ((x) * 2)` + `const long K = TWICE(3);` | `[IDL0002] expected ';' after the constant` (1:22) — function-like macros are not expanded |
| `#define CAT(a, b) a##b` + `struct CAT(S, a) { … };` | `[IDL0002] expected '{' opening the struct body` (1:12) — `##` |
| `#define STR(x) #x` + `const string K = STR(ab);` | `[IDL0002] expected ';' after the constant` (2:4) — `#` as the stringize operator |
| `#define NN \` with a continuation line `4` | `[IDL0001] expected a constant expression` (3:12) — backslash line continuation |
| `#define AA 1` + `#define BB AA` + `const long K = BB;` | `[IDL0006] macro 'BB' expands to 'AA', which is itself a macro; nested expansion is not performed` |
| `#if 2 * 3 > 5` | `[IDL0006] '#if 2 * 3 > 5' is not a form this preprocessor evaluates` |
| `const long K = __LINE__;` | `[IDL0010] a name does not resolve: … ["_LINE__"]` — there are no predefined macros |
| `const string K = __FILE__;` | `[IDL0010] a name does not resolve: … ["_FILE__"]` |

What does work, recorded here so that it stays that way: `#include` in
both forms, `#define` without parameters, `#undef`, `#ifdef`, `#ifndef`,
`#if`/`#else`/`#elif`/`#endif` with a constant, `defined(X)`, `#line`,
`#pragma`, `#pragma once`.

**Probes:** `lex-7.3-define-function`, `lex-7.3-token-paste`,
`lex-7.3-stringize`, `lex-7.3-line-continuation`, `lex-7.3-nested-macro`,
`lex-7.3-if-arithmetic`, `lex-7.3-predefined-line`,
`lex-7.3-predefined-file`

---

## B — accepted although the specification requires a rejection

There is no error message here, there is a result. In the first three
rows that result was demonstrably wrong; those three are closed.

### B.1 §7.4.1.4.3 — the declared type of a constant (closed)

**Spec:** §7.4.1.4.3 "Constants" — a constant declaration consists, among
other things, of "a value expression (`<const_expr>`), which shall be
consistent with the type declared for the constant". §7.2.6.3 —
"Attempts to assign a wide string literal to a non-wide string constant
or to assign a non-wide string literal to a wide string constant result
in a compile-time diagnostic."

The declared type was discarded; the literal's type replaced it:

| IDL | `check` before | emitted Rust before | `check` now |
|---|---|---|---|
| `const long K = "x";` | 0 errors | `pub const K: &'static str = "x";` | **[IDL0019]** |
| `const string Ss = L"x";` | 0 errors | `pub const Ss: &'static str = "x";` | **[IDL0019]** |
| `const wstring Ss = "x";` | 0 errors | `pub const Ss: &'static str = "x";` | **[IDL0019]** |

Someone who wrote `const long` and got a `&str` learned about it from the
Rust compiler at the use site, not here.

**The fix.** `Lowering::check_const_value_form` in
`crates/idl-lower/src/lower.rs` compares the form of the value with the
declared type. The comment at that spot already claimed the rule — "the
expression is checked against it rather than used to derive it" — with no
check underneath it. New diagnostic code `IDL0019 TypeMismatch`; codes
are appended, never renumbered.

**Where the check stays silent, and why.** It fires only when the
declared type resolves here to a primitive, a `string`/`wstring` or a
`fixed`, *and* the expression settles to one literal form. A named type,
an alias, an enumeration, a reference to another constant and an
unresolved reference all pass without a word. A rejection that reaches
too far breaks valid IDL, which costs more than the defect it removes.
Measured: none of the 748 committed `*.idl` files reports IDL0019.

**Tests:** `crates/idl-lower/tests/validation.rs` —
`a_constant_whose_value_is_not_of_its_declared_type_is_refused`,
`the_wide_and_narrow_string_pair_is_the_case_the_clause_names` and
`every_form_the_specification_admits_still_gets_through` (28 forms that
have to keep getting through). Mutation: `if admitted || true` turns the
first two red, `_ => (false, …)` in place of `_ => return` the third.

**Probes:** `scope-7.4-const-type-mismatch`,
`scope-7.2.6.3-wide-to-narrow`, `scope-7.2.6.3-narrow-to-wide` — all
three now `err` in all nine columns.

### B.2 §7.5.2 — three scoping rules (one closed, two deliberately open)

**Spec:** §7.5.2 — "The name of a module, structure, union, map,
interface, value type, event type, exception or home may not be redefined
within the immediate scope of the module, …". The specification gives
both cases as examples with the comment `// Error`.

| IDL | Spec says | before | now |
|---|---|---|---|
| `module Ma { typedef short Ma; };` | Error (example §7.5.2) | 0 errors | **[IDL0008]** |
| `interface Ia { void ia(in short jj); };` | Error (example §7.5.2) | 0 errors | 0 errors — open |
| `module Inner2 { typedef Inner1::Ss1 Ss2; typedef string inner1; };` | Error (example §7.5.2) | 0 errors | 0 errors — open |

**The first is closed.** `check_redefines_enclosing_scope` in
`crates/idl-lower/src/lower.rs`, called from `check_collision`, compares
a declaration's name with the name of the scope it is written in —
case-insensitively, because §7.2.3 makes every identifier comparison in
IDL so.

**Immediate scope only**, and that is the whole of the rule's safety:
`module Ma { module Mb { typedef short Ma; }; };` stays legal, because
`Ma` is not the name of the scope `Mb`. The difference between this rule
and one that breaks valid IDL is exactly there.

The rule confirms a decision the tree had already made: commit
`aab2fe27404` recorded that `module common { struct common { … } }` is
not valid IDL and corrected comments in the Java and TypeScript adapters
that claimed the opposite. The interop case
`tests/interop-corpus/resolve-module-named-as-type/type.idl` measures
which vendors refuse it; it is now on the corpus test's exception list.

**Two stay open, with reasons.**

- **The operation that clashes with the interface name.** §7.5.2 lists,
  for the redefinition rule, "module, structure, union, map, interface,
  value type, event type, exception or home" — operations are not among
  them; that case comes from the introduction example. Whether the rule
  covers every `<export>` or only operations is interpretation, and
  reaching too far breaks CORBA interfaces.
- **The "identifier introduction" rule.** It requires tracking every use
  of a name per scope, case-insensitively. That is the highest
  over-reach risk on the whole list. Both counter-probes
  (`scope-7.5.2-not-introduced-ok`, `scope-7.5.2-rooted-no-introduce`)
  pass today — correctly, but for the wrong reason, because the rule does
  not exist. An implementation needs considerably more counter-probes
  than those two.

What §7.5.2 otherwise **does** hold: module reopening, redefinition in a
nested scope, `enum` names in the enclosing scope
(`scope-7.5.2-enum-name-in-scope` is correctly refused), and the case
collision rule of §7.2.3, which even checks identifiers against keywords.

**Tests:** `crates/idl-lower/tests/validation.rs` —
`a_declaration_may_not_reuse_the_name_of_its_own_scope` and
`only_the_immediate_scope_is_compared`.
Mutation: switching the condition off (1 red); replacing the scope name
with the declaration's own, so every declaration is hit (33 red).

**Probes:** `scope-7.5.2-redefine-own-name` — now `err`;
`scope-7.5.2-op-clashes-iface` and `scope-7.5.2-introduced-ident` stay
`ok`.

### B.3 §7.4.1.4.4 — direct recursion (closed)

**Spec:** §7.4.1.4.4.4.4 — a structure may contain itself only across a
sequence (or an `@optional` member, or a map).

**IDL:** `struct Sa { Sa m; };`

**Before:** `check` reported 0 errors. `generate` reported
`SpecRefuses { detail: "NodeId(3) closes a type cycle with nothing on it
that terminates a value — no sequence, no map and no @optional member —
which IDL 4.2 §7.4.1.4.4.4.4 does not admit; …" }`.

So the rule was there and cited the clause; it just sat in the layout
stage rather than the front end. Someone using `check` to find out
whether a file is valid IDL did not find out.

**The fix — extracted, not rebuilt.**
`zerodds_ast_layout::types_that_close_an_inadmissible_cycle` runs the
same `plan` walk and keeps exactly that one error. It is explicitly
**not** a second implementation of the cycle test: the barrier
bookkeeping — a cycle is legal exactly when a sequence, a map or an
`@optional` member lies on it — lives in the descent, and a rule written
twice drifts. Every other `LayoutError` is dropped: an unresolved
reference is the resolve pass's to report, and an encoding that cannot
carry a type is `generate`'s.

It is called from `zerodds_idl_compose::compose_source_with`, after the
resolve pass (a cycle is a property of the resolved type graph). Every
caller of the front end therefore gets the same answer — `check`, a
`build.rs` through `zerodds-build`, a wire probe — rather than only the
CLI. `ast-layout`'s own behaviour is unchanged.

The message names the type in full. The layout stage's named a `NodeId`,
which outside a debugger is worse than nothing.

**Admissible recursion stays admissible,** and that is the more expensive
half of the rule: `sequence<Sa>`, `sequence<Sa, 4>`, `@optional Sa`,
`map<long, Sa>` and mutual recursion across sequences all still get
through. The interop corpus holds seven recursion cases; exactly one —
`recursive-array` — is the inadmissible form, and the test reads all
seven off disk rather than restating them.

**Tests:** `crates/idl-compose/tests/recursion.rs`, four tests.
Mutation: commenting the call out (3 red), keeping the wrong
`LayoutError` in the extractor (3 red).

**Probe:** `scope-7.4-recursive-struct` — now `err`.

### B.4 §7.4.13 — seven bounds on bit sets and bit masks (closed)

**Spec:** §7.4.13.4.3.2 — "The first one (`<positive_int_const>`) is the
number of bits that can be stored (its size). The maximum value is 64";
"When the destination type is given, the number of stored bits cannot
exceed its size (i.e., 1 for boolean, 8 for octet, …)". §7.4.13.4.3.3 —
"A bit mask must contain at least one value and no more than its size
expressed in bits"; "By default, the size of a bit mask is 32";
`@bit_bound` "must be lower than or equal to 64. Accordingly the number
of values cannot then exceed the value given to @bit_bound";
`@position` "Possible positions range from 0 … up to (size – 1)".

| IDL | before | now |
|---|---|---|
| `bitset Ba { bitfield<65> f1; };` | `check` 0 errors; only the C writer refused: `NoWriter { … "bitset wider than the sixty-four bits an anchor holds" }` | **[IDL0014]** |
| `bitset Ba { bitfield<9, octet> f1; };` | 0 errors, emitted in both languages | **[IDL0014]** |
| `bitset Ba { bitfield<2, boolean> f1; };` | 0 errors, emitted in both languages | **[IDL0014]** |
| `@bit_bound(65) bitmask Ba { F1 };` | 0 errors | **[IDL0014]** |
| `@bit_bound(2) bitmask Ba { F1, F2, F3 };` | 0 errors | **[IDL0014]** |
| `bitmask Ba { F01 … F33 };` (33 values, default size 32) | 0 errors | **[IDL0014]** |
| `@bit_bound(2) bitmask Ba { @position(5) F1 };` | 0 errors | **[IDL0014]** |

The one bound the front end did hold is the lower one: `bitmask Ba { };`
is refused with `[IDL0002] a 'bitmask' needs at least one bit value`.

**The fix.** `Lowering::check_bit_bounds` in
`crates/idl-lower/src/complete.rs`, called from `complete()` rather than
from the declaration: `@bit_bound` and `@position` are annotations and
are not applied until the file is fully lowered. Diagnostic code
`IDL0014 InvalidBound`, the existing code for a bound outside its legal
range.

The destination widths are the ones §7.4.13.4.3.2 names: 1 for
`boolean`, 8 for `octet`/`int8`/`uint8`, 16, 32, 64. A destination type
that does not resolve here, or is none of those, produces no diagnostic —
the same rule as in B.1: stay silent where the answer is not settled.

**One file in the tree is affected.**
`tools/wire-oracle/cases/bitmask_past_bound/type.idl` writes
`@bit_bound(4)` with `@position(7)`, deliberately: the case measures what
other vendors' generators do with a position outside the range.
`tools/byte-gate/gate.sh` hands it only to Cyclone, Fast DDS, OpenDDS and
RTI — the ZeroDDS side does not compile it — so the measurement it
carries is unchanged. No test in the tree runs `check` over every
committed `.idl`.

**Tests:** `crates/idl-lower/tests/validation.rs` — five tests, one per
rule, plus `the_bit_forms_the_specification_admits_still_get_through`
with nine forms that have to keep getting through.
Mutation: `if false && …` in front of each of the four comparisons turns
one test red apiece; `MAX_BITFIELD_BITS`/`MAX_BIT_BOUND` from 64 to 63
turns three red.

**Probes:** `sem-7.4.13-bitfield-over-64`,
`sem-7.4.13-bitfield-dest-octet`, `sem-7.4.13-bitfield-dest-bool`,
`sem-7.4.13-bitbound-over-64`, `sem-7.4.13-bitmask-over-bound`,
`sem-7.4.13-bitmask-over-default`, `sem-7.4.13-position-out-of-range` —
all seven now `err`.

### B.5 §7.4.15.4.2 — four rules on applying an annotation (closed)

**Spec:** §7.4.15.4.2 — "Members with no default value must be given a
value"; "A constant expression, whose type must be compatible with the
member's declaration"; "In case there is only one member, the annotation
application may be shortened to the name of the annotation prefixed with
@ and followed with the constant value of that unique member"; and
finally "malformed definitions shall be treated as an error".

| IDL | before | now |
|---|---|---|
| `@annotation Aa { long v; };` + `@Aa struct …` — required member, no value | 0 errors | **[IDL0007]** |
| `@Aa("x")` on `long v default 0` — type does not fit | 0 errors | **[IDL0007]** |
| `@Aa(nosuch = 1)` — no such member name | 0 errors | **[IDL0007]** |
| `@Aa(1)` on an annotation with two members | 0 errors | **[IDL0007]** |

**The defect underneath was bigger than the four rules.** None of them
could hold, because an annotation had no members in the model at all.
Rule (222) writes an annotation member's name as a `<simple_declarator>`,
and `parse_annotation_decl` reads it with a bare `expect(Ident)` and no
`Declarator` node. `Lowering::declared` walks `Declarator` children,
found none, returned an empty list — and every member of every
annotation vanished without a word. `@annotation Aa { long v; };`
reached the model with zero members.

`annotation_decl` now reads the name directly (`first_ident` reads the
node's own tokens and not its descendants', so the name of `Ea v;` is `v`
and not `Ea`) and reports a member with no name instead of dropping it.

**And the body is its own scope.** Rule (221) admits an `enum`, a `const`
and a `typedef` beside the members —
`@annotation Mode { enum Kind { READ, WRITE }; Kind kind default READ; };`.
Those were parsed and never lowered, which nothing noticed while the
members themselves were being dropped. With the members read, a member's
type had nothing to resolve against:
`crates/idl-parse/tests/fixtures/spec_features/user_annotations.idl` fell
over at once. `annotation_decl` now pushes a scope and lowers the three
admitted forms inside it.

**The boundary against the ignore rule is the clause's own.** The same
§7.4.15.4.2 requires an *unknown* annotation to be ignored. None of the
four rules therefore reaches an annotation the file did not declare:
`check_annotation_application` is called only with a resolved
declaration. `@RTI_topic_kind(NO_KEY)` is untouched.

**The type check is the one from B.1.**
`Lowering::value_disagrees_with` answers both clauses — §7.4.1.4.3 for a
constant's value and §7.4.15.4.2 for an annotation argument. One reading,
so the two cannot drift apart.

**Left open because nothing settles it:** whether an enumerator declared
inside an annotation body may be written unqualified at an application
site — `@Mode(WRITE)`. Neither §7.5 nor rule (221) says, and no fixture
in the tree exercises it. So no assertion about it stands in the test.

**Tests:** `crates/idl-lower/tests/validation.rs` — five tests, one per
rule plus `none_of_the_four_rules_reaches_an_unknown_annotation` and
`an_annotations_declared_members_reach_the_model`;
`tools/idlc/tests/annotation_arguments.rs` for the body scope.
Mutation: switching each of the four conditions off in turn (1 red
apiece), removing the call entirely (4 red).

**Probes:** `sem-7.4.15-missing-required`,
`sem-7.4.15-param-type-mismatch`, `sem-7.4.15-unknown-param-name`,
`sem-7.4.15-shorthand-two-members` — all four now `err`.
`sem-7.4.15-unknown-annotation` and `-unknown-annotation-args` stay `ok`;
they are the counter-probes and not a finding.

### B.6 §7.3 — `#error` (closed)

**Spec:** IDL 4.2 §7.3 adopts the preprocessor of ISO/IEC 14882:2003
wholesale. Its §16.5: "#error causes the implementation to produce a
diagnostic message that includes the specified sequence of preprocessing
tokens."

**IDL:** `#error stop here`

**Before:** `check` reported 0 errors; `generate --lang rust` wrote a
file with no content. The directive was passed over in silence — the one
directive that exists to stop a build was the only one that could not.

**Now:** `[IDL0020]`, with the tokens in the message as the clause
requires. A new diagnostic code, appended; `IDL0019` stays
`TypeMismatch`, because codes are never renumbered.

**The rule's only subtlety** is `if keeping`: an `#error` inside a branch
the conditionals cut out is text that was never reached. Reporting it
would refuse every file that guards one — which is nearly every file that
has one at all.

**Tests:** `crates/idl-parse/tests/preprocess.rs` — three tests, among
them `an_error_directive_in_a_branch_that_is_cut_out_says_nothing` over
`#ifdef`, `#else` and `#if 0`.
Mutation: not reporting the directive (2 red); removing `if keeping`, so
it reports in the dead branch too (1 red).

**Probe:** `lex-7.3-error-directive` — now `err`.

---

## C — recognized, not emitted

All eleven rows are **deliberately refused**: in every case the backend
names the reason and, where there is one, the clause. That is not a gap
in the sense of "overlooked", but it has to stay distinguishable, because
an application that uses the construct gets nothing.

| Construct | Production | Rust | C | Reason, as the tool states it | Fixtures |
|---|---|---|---|---|---|
| `any` as a member | 69, 70 | — | — | `NoLanguageForm { construct: "any" }`, in C additionally `NoWriter { … "'any', which DDS-XTypes has no encoding for; only CORBA CDR writes one, as a TypeCode ahead of the value" }` | 9 |
| `typedef any` | 70 | — | — | ditto | 1 |
| `any` as an annotation member | 223, 224 | ok | — | as above | – |
| `Object` | 117, 118 | — | — | `NoLanguageForm { construct: "Object" }` | 4 |
| `ValueBase` | 131, 132 | — | — | `NoLanguageForm { construct: "ValueBase" }` | 1 |
| `fixed<9,2>` | 42 | — | ok | `NoWriter { member: "m", kind: "fixed" }` | 17 |
| `valuetype Va string;` (value box) | 125, 126 | ok | — | `NoWriter { … "boxed value, whose presence flag has no settled wire form" }` | 1 |
| `abstract valuetype` | 125, 127 | ok | — | `NoLanguageForm { … "an aggregate with no members — C23 §6.7.2.1p8 leaves a structure with no named member undefined" }` | 0 |
| `abstract eventtype` (definition) | 166, 168 | ok | — | ditto | – |
| `abstract eventtype` (forward + definition) | 167 | ok | — | ditto | 1 |
| `struct Sa { };` (empty struct, BB Extended Data-Types) | 195 | ok | — | ditto | 3 |

**Closed:** the union with no `default:` whose defaulted discriminator
selects no branch (probe `bb1-050-union-no-default`, productions 49, 50)
is `ok` in the Rust column. The union's `enum` has the variant
`_NoMember(<discriminator>)` for a discriminator that selects no member
(IDL 4.2 §7.4.1.4.4.4), and a variant whose case does not fix the
discriminator (several labels, `default:`) carries it ahead of the
member. Test:
`crates/lang-rules/tests/rust_corpus_compiles.rs::a_union_keeps_its_discriminator_through_rustc_and_back`.

The last four share a single cause: C has no form for an aggregate with
no members, and `abstract valuetype`, `abstract eventtype` and the empty
struct of production 195 all run into it. Rust emits all four.

The 17 fixtures with `fixed<…>` and the 9 with `any` are the hint that
both constructs occur in the tree but do not go through the Rust
emission.

### C.2 The interface family in C, the object type at ports and connectors in Rust

63 more probes, all refused with a reason:

- **C, 62 probes** — every probe with an `interface`, including one that
  only hangs off a component, a home or a port type: "C has no complete
  DDS-RPC 1.0 binding for an interface, and this model declares 1 …",
  with the list of columns that have one. Since `7b651ac2643` an
  interface binding refuses what it cannot place in full rather than
  emitting part of it. Probes: `bb3-074` to `bb3-095`,
  `bb4-097-export-*`, `bb5-103-value-supports`, `bb6-113-typeid`,
  `bb6-114-typeprefix`, `bb6-119-local-interface`, `bb6-120-oneway`,
  `bb6-123-op-with-context`, `bb7-129`, `bb7-130`, `bb8-135` to
  `bb8-143`, `bb9-145` to `bb9-150`, `bb10-155` to `bb10-165`,
  `bb11-173` to `bb11-182`, `ann-8.3.6.1-service*`,
  `ann-8.3.6.2-oneway`, `ann-8.3.6.3-ami*`,
  `scope-7.5.2-op-clashes-iface`, `scope-7.5.3-type-name-lookup`.
- **Rust, 12 probes** — a port (`provides`, `uses`, `port`,
  `mirrorport`), a port type or a connector holds an object reference,
  and the Rust column has no form for `Object`:
  `NoLanguageForm { construct: "Object", language: "Rust" }`. Probes:
  `bb8-141-provides`, `bb8-143-uses`, `bb10-157-interface-type-obj`,
  `bb10-158-uses-multiple`, `bb11-173` to `bb11-182`.

Not determined: which commit after the last recording moved the twelve
Rust probes from `ok` to `err`; the refusal of `Object` itself is older.

---

## G — `format` refuses what `check` accepts

`zerodds-idlc format` reads the token stream before the preprocessor, so
that comments and directives survive. An object-like macro stands there
as an identifier and is valid syntax; the invocation of a function-like
macro is not. `format` therefore refuses the file — with a diagnostic,
losing nothing — while `check` accepts it after expansion.

| IDL | `format` |
|---|---|
| `#define TWICE(x) ((x) * 2)` + `const long K = TWICE(3);` | `[IDL0002] expected ';' after the constant` |

**Probes:** `lex-7.3-define-function`, `lex-7.3-token-paste`,
`lex-7.3-stringize`.

**Closed:** `zerodds-idlc format` decides validity as `check` does, after
the preprocessor, and then lays the text out over the source's tokens
(`format::canonical_of_checked`); a macro call stays the token sequence
it was written as. The three probes are `ok` in `format` and in the
`trip` round trip now. Test:
`tools/idlc/src/format.rs::tests::a_function_like_macro_call_keeps_its_tokens`.

---

## D — emitted, but no `.idl` in the tree contains it (closed)

Both backends produce source, and none of the committed `*.idl` files
carried the construct. After the re-recording there were 26. Every one
now has a fixture under `tools/idlc/tests/idl-4.2-gaps/`, one per probe
and named after it, and `tools/idlc/tests/idl_4_2_gaps.rs` runs them
through `zerodds-idlc check` and `zerodds-idlc generate --lang rust` and
`--lang c`. Each fixture carries the probe's own IDL, so the text
answers the fixture question `scan.sh` asks with its `witness` column.

Eleven had left earlier: `@range`, `@min`, `@max`, `@unit`, `const octet`
and `union Ua;` had gained a fixture in the tree; `@ami`,
`interface Ia;`, `interface Ia : Ib`, `local interface` and `context`
are no longer emitted by the C column (C.2). Four had joined that did not
pass `check` before: adjacent string literals, `typedef struct … Ali;`,
the function-like macro and `##`.

The 26 that left:

| Construct | Production / clause | Probe |
|---|---|---|
| `const fixed` | 6, 43 | `bb1-006-const-fixed` |
| `const wchar` | 6 | `bb1-006-const-wchar` |
| `const wstring` | 6 | `bb1-006-const-wstring` |
| `^` in a constant expression | 9 | `bb1-009-xor-expr` |
| `>>` in a constant expression | 11 | `bb1-011-shift-right` |
| `~` as a unary operator | 15 | `bb1-015-unary-tilde` |
| fixed-point literal | 17 | `bb1-017-lit-fixed` |
| wide string literal | 17 | `bb1-017-lit-wstring` |
| adjacent string literals | 17 | `bb1-017-lit-string-concat` |
| `typedef struct … Ali;` | 64 | `bb1-064-typedef-constr` |
| `valuetype Va;` (forward) | 99, 110 | `bb5-110-value-forward` |
| `int64` | 212 | `bb13-212-int64` |
| octal character escape | 7.2.6.2 | `lex-7.2.6.2-char-escape-octal` |
| hexadecimal character escape | 7.2.6.2 | `lex-7.2.6.2-char-escape-hex` |
| Unicode escape | 7.2.6.2 | `lex-7.2.6.2-wchar-escape-unicode` |
| floating point, leading dot | 7.2.6.4 | `lex-7.2.6.4-float-leading-dot` |
| floating point, trailing dot | 7.2.6.4 | `lex-7.2.6.4-float-trailing-dot` |
| fixed-point literal | 7.2.6.5 | `lex-7.2.6.5-fixed-literal` |
| function-like macro | 7.3 | `lex-7.3-define-function` |
| `#ifndef` | 7.3 | `lex-7.3-ifndef` |
| `#if`/`#else` | 7.3 | `lex-7.3-if-else` |
| `#elif` | 7.3 | `lex-7.3-elif` |
| `defined(X)` | 7.3 | `lex-7.3-defined-operator` |
| `#undef` | 7.3 | `lex-7.3-undef` |
| `##` | 7.3 | `lex-7.3-token-paste` |
| `#line` | 7.3 | `lex-7.3-line-directive` |

Three of them were not measurable with the `witness` search before: the
patterns of the two character escapes and of the Unicode escape in
`cases.tsv` were over-escaped (`'\\\\[0-7]`, `'\\\\x`, `\\\\u…`) and
did not match even the probe's own IDL. They now stand as `'\\[0-7]`,
`'\\x` and `\\u[0-9a-fA-F]` in the corpus.

---

## E — `--strict` (closed)

`tools/idlc/README.md` and `zerodds-idlc --help` describe `--strict` as
"OMG IDL 4.2 only; reject every vendor extension". The switch was parsed
and then dropped — `tools/idlc/src/main.rs`, `analyse()`:
`let _ = options.strict;` — and `Parser::over()` wired
`Dialect::ALL_VENDORS` in regardless. Across all probes there was not one
where `check` and `check --strict` differed.

**The path the dialect now takes.** `Options::dialect()` in
`tools/idlc/src/main.rs` is the one place `--strict` is read, and every
one of the six subcommands goes through it — five via `analyse()`,
`format` via `format::canonical_under`. The dialect reaches both stages
vendor constructs live in: `zerodds_idl_parse::parse_preprocessed_with`
for the grammar and `zerodds_idl_lower::Options::dialect` for the
pragmas.

**Measured, after the fix:**

| IDL | Vendor | `check` | `check --strict` |
|---|---|---|---|
| `keylist Sa (m);` | RTI Connext | 0 errors, 1 warning | **1 error** |
| `#pragma keylist Sa m` | Cyclone DDS | 0 errors, 1 warning | **1 error** |
| `#pragma DCPS_DATA_TYPE "Sa"` | OpenDDS | 0 errors, 1 warning | **1 error** |
| `#pragma DCPS_DATA_KEY Sa m` | OpenDDS | 0 errors, 1 warning | **1 error** |
| `@Key` (the Fast DDS spelling) | — | 0 errors | 0 errors |

The three directives are the ones the front end **acts on**: all three
make members keys, so a front end that ignores them builds a different
type from the same file. That is exactly the question `--strict` answers.

**`@Key` stays admitted, and that is not a remainder of the gap.** IDL
4.2 §7.4.15.4.2: "in case an unknown annotation is encountered, it should
be ignored by the IDL-processing tools."
`crates/idl-lower/src/annotation.rs` recognizes annotations
case-sensitively, so `@Key` is not a known annotation, the front end does
not act on it, and refusing it under `--strict` would break conforming
IDL to gate nothing. The same holds for `@RTI_*` and `@RTPS_*`. Had
`@Key` been read as a key, it would be in the table above.

**Tests:** `tools/idlc/tests/strict.rs` — the matrix of six subcommands ×
four vendor extensions × two dialects, plus the counter-check that the
same files get through every subcommand without `--strict`;
`crates/idl-lower/tests/dialects.rs`,
`strict_rejects_every_vendor_extension_the_front_end_acts_on` and
`strict_does_not_refuse_an_unknown_annotation`.

**Probes:** `dialect-rti-keylist`, `dialect-cyclone-pragma-keylist`,
`dialect-opendds-pragma-datatype` — `strict` now `err`;
`dialect-fastdds-key-capital` stays `ok`.

**One probe was built wrong.** `lex-7.3-pragma` stood for §7.3 — for the
preprocessor handling `#pragma` at all — and used `#pragma keylist Sa m`
to do it, a Cyclone directive. It measured two axes at once and went red
under `--strict` for a reason that has nothing to do with §7.3. It now
writes `#pragma vendorless Sa`.

---

## F — `format` accepted what `check` refused (closed)

Not a finding about a construct but about a subcommand. It only showed up
when the survey went from two columns to the matrix over all six.

`zerodds-idlc format` did not run the pipeline `check` runs.
`tools/idlc/src/format.rs`, `canonical()`, calls
`zerodds_idl_parse::parse(source)` — the bare parser. No preprocessor, no
lowering stage, no resolve stage. The formatter refused a file that did
not *parse*; everything that only showed up after that went through
without a word. Measured: **21 of the 356 probes** had `check err` and
`format ok`, and over the committed tree 12 of the 748 `*.idl` files. The
user got a formatted file back that their tool discarded on the next
step.

| Stage that reports the error | Probes |
|---|---|
| Preprocessor | `lex-7.3-line-continuation`, `lex-7.3-nested-macro`, `lex-7.3-if-arithmetic`, `lex-7.3-predefined-line`, `lex-7.3-predefined-file` |
| Lowering (lexical/scoping) | `lex-7.2.3-case-collision`, `lex-7.2.3-keyword-collision`, `scope-7.5.2-enum-name-in-scope`, `scope-7.4-dup-case-label`, `scope-7.4-two-defaults`, `scope-7.4-enum-dup` |
| Lowering (semantic) | `sem-7.4.13-bitmask-empty`, `sem-7.4.1-seq-bound-zero`, `sem-7.4.1-string-bound-zero`, `sem-7.4.1-array-size-zero`, `sem-7.4.1-array-size-negative`, `sem-8.3.1.1-id-collision`, `sem-8.3.1.2-autoid-bad-arg`, `sem-8.3.1.6-ext-bad-arg`, `sem-8.3.1.3-key-and-optional` |
| Resolve | `scope-7.4-unknown-type` |

**The fix.** `run_format` in `tools/idlc/src/main.rs` now goes through
`analyse()` first — the pipeline `check` runs — and refuses the file as
soon as an error is reported. Nothing is printed on the success path: the
formatted text goes to stdout, and a warning ahead of it would land in
the file a caller redirected it into.

All 21 probes now stand at `format err`. The library function
`format::canonical_under` keeps its parse-level contract; the corpus test
in `tools/idlc/src/format.rs` still measures through it.

**The other direction does not occur:** no probe has `check ok` and
`format err`. `lint`, `dump-ast` and `print-deps` go through `analyse()`
the way `check` does and agree with it on all 357 probes.

**Round trip:** over the 325 probes both `check` and `format` accept it
holds completely — same model, same dependencies, idempotent format. No
silent loss in the formatter. That explicitly includes the 26 probes
that made up category D until this fixture work: the formatter loses
none of them.

## The tree itself

The survey measures probes. Up to here this page said nothing about
whether the tree's own 748 committed `*.idl` files are valid IDL — and
there was no test running `check` over all of them. A construct that
went wrong in one of our own files only showed up if some other test
happened to read it.

What surfaced it was `tools/wire-oracle/cases/bitmask_past_bound/type.idl`:
deliberately out of spec, handed only to other vendors' generators, and
indistinguishable from a file that had quietly gone wrong. It took a new
rule (B.4) to make it visible at all.

`tools/idlc/tests/corpus.rs` now runs `check` over every committed
`.idl`. **13 files** are refused, all 13 with a written reason in
`INTENDED` — the allowed codes per file, and a sentence saying what the
file is for. Three groups:

| Group | Files | What they are |
|---|---|---|
| Probe inputs | 6 | `tools/hashid-gate` (2), `tools/inventory-probes` (2), `tools/try-construct-gate` (2) — the refusal *is* the measurement |
| Grammar fixtures | 4 | `ccm_components`, `corba_value_repository`, `fastdds_pragmas`, `zerodds_security` — curated for syntax coverage, not for a model that closes |
| Wire measurement | 1 | `bitmask_past_bound` — asks four other vendors what they do with a forbidden `@position` |
| ORB corpus | 1 | `05-typecode` — `CORBA::TypeCode`, which every ORB predeclares |
| RTI fixture | — | was the 13th; passes since A.4 |

A second test checks the other direction: an entry whose file passes
again is stale and turns the test red. An exception with no reason it
refuses outright.

**One real defect came out of it.**
`examples/demos/perf-camera-dds/idl/camera.idl` wrote `unsigned octet
quality_hint;` — `unsigned octet` is in no production of IDL 4.2, and
`octet` is unsigned already. A shipped example no conforming front end
compiles. Corrected to `octet quality_hint;`.

---

## What this survey does not cover

- **Emission in two languages only.** `rust` and `c`. The remaining
  backends under `crates/lang-rules/src/default_*.rs` are unchecked.
- **No wire comparison.** What was measured is whether source is
  produced, not whether the resulting bytes are right. That is the
  question the conformance fixtures answer, not this page.
- **`fixtures` is a text search.** A hit does not prove the file is used
  in a test. A zero proves the construct does not occur in the tree.
- **No comparison against other front ends.** Whether `fastddsgen`,
  Cyclone's `idlc` or `tao_idl` accept the same IDL is not measured.
- **Clause 9 (profiles).** The four CORBA/CCM and three DDS profiles are
  compositions of the building blocks; whether `zerodds-idlc` needs a
  profile argument is a product question and not a gap against the
  grammar. Unchecked.
- **Table 7-6 as a whole.** The specification lists 84 keywords. One
  representative is checked (`typedef boolean BOOLEAN;` is rejected with
  `[IDL0017]`) along with the escape form (`long _long;` is accepted, and
  150 fixtures in the tree use it). The other 83 keywords are not probed
  individually as identifiers. §7.2.1 Tokens and §7.2.5 Punctuation are
  likewise not covered character by character.

## The tool

```
tools/idl-4.2-gap-scan/scan.sh              measure and compare
tools/idl-4.2-gap-scan/scan.sh --record     rewrite expected.tsv
tools/idl-4.2-gap-scan/scan.sh --report     print the full measurement
IDLC=/path/to/zerodds-idlc tools/idl-4.2-gap-scan/scan.sh
```

Exit codes:

| | |
|---|---|
| 0 | measured; the gap is exactly what `expected.tsv` records |
| 1 | a probe that used to get through no longer does — the gap grew |
| 2 | nothing measured: no `zerodds-idlc`. Never a pass. |
| 3 | a probe now gets through that `expected.tsv` records as failing — the gap shrank and this page is behind the tree |
| 4 | `cases.tsv` and `expected.tsv` name different probes |

Runtime on an M1: 36 seconds for 356 probes over 748 `.idl` files.
Before the matrix it was 16 seconds with two subcommands measured; the
eleven invocations per probe cost the rest.

Exit 3 is not cosmetic. A negative list that still carries a closed gap
as open is worse than none: it costs trust in the remaining rows.

## Method note

Transferable to CORBA 3.3, the DDS specs and the 47 own specs under
`docs/specs/` — at very different costs.

**What was cheap.** The denominator. IDL 4.2 has Annex A: a consolidated,
numbered grammar from which `pdftotext -layout` and twenty lines of
Python pull all 227 productions without a hole (gap check: `range(1, 228)`
against the parsed numbers). Without that annex, gathering the
productions out of fifteen building-block clauses would have cost a
multiple, and completeness would not have been checkable.

**What was expensive.** The probes. 356 minimal `.idl` files written by
hand, and two rounds of rework, because the first versions failed for the
wrong reason: an identifier `Alias` collides with the keyword `alias`,
and a union with no `default:` is refused by the backend for a reason
that has nothing to do with the production under test. Every mismeasure
looked like a finding. That is the actual work in a survey like this:
telling apart whether the probe or the tree is wrong.

**Where the full text is needed.** For categories A, C and D the grammar
is enough. For category B — accepted although forbidden — the prose is
needed, verbatim. All 19 B rows come from sentences that appear in no
production: "Adjacent string literals are concatenated", "The maximum
value is 64", "Members with no default value must be given a value".
Without the PDF in full they would have been unfindable. And the same
clause refuted two claims that would otherwise have been written down as
gaps: unknown annotations *should* be ignored, and `struct S { long s; }`
*is* an error. Without the text those would have been two wrong rows.

**What that means for the other specs.**

| Corpus | Denominator available? | Estimated effort |
|---|---|---|
| CORBA 3.3 | Yes — Part 1 Annex A carries the same consolidated grammar, plus the GIOP/IIOP message types as tables. The prose, though, runs to about 1,150 pages across three parts, eight times IDL 4.2. | The grammar axis is roughly as here, one to two days. The B axis (forbidden forms) is the cost driver and has to be done per part. The probes need a different driver, because `zerodds-idlc` does not cover the whole surface: for GIOP/POA/IR the probe has to be a Rust test, not an `.idl` file. |
| DDS specs (DCPS 1.4, XTypes 1.3, DDSI-RTPS 2.5, Security 1.2) | Not in this form. The denominator there is not a grammar but a set of tables: QoS policies, submessage kinds, TypeKinds, plugin operations. They can be pulled out of the PDF mechanically, but each spec needs its own extractor. | One day per spec for the extractor plus the probes. The upside: the probes here are bytes, not text files, and `tools/byte-gate` and `tools/unspecified-reach` are the drivers that already exist. The four categories become: does the decoder know the field / does the encoder write it / is there a test vector / is it deliberately not implemented. |
| Own specs under `docs/specs/` | Yes, and cheaper than all the others — the 47 documents are Markdown with numbered clauses, in the tree. One extractor for all 47. | The denominator is a matter of hours. The catch is a different one: a spec written in-house cannot be wrong against itself. What a negative list finds there is a clause with no implementation — useful, but not the same as a gap against a foreign denominator. The yield ranks behind CORBA and the DDS specs. |

**The order I would recommend.** DDS-XTypes 1.3 before CORBA 3.3: the
denominator is smaller, the probes are bytes rather than text, and the
wire area is the one where a missed construct silently delivers wrong
data instead of an error message. The own specs last.

**One rule the survey itself taught.** "It does not occur in the tree" is
no evidence for "it is not allowed" — that was the trigger. But the
converse holds too: "the front end rejects it" is no evidence for "the
specification forbids it". Both directions need the spec text, and both
came up in this survey.

## Decision records

None. This page surveys and closes nothing.
