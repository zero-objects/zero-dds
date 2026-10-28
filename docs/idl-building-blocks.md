# Every construct in OMG IDL 4.2, and who can compile it

OMG IDL 4.2 is not one language. §7.4 divides it into **fifteen building
blocks**, rules (1) to (227), and §9 names the **profiles** — sets of
building blocks — that middleware is written against. Most
implementations pick a profile.

**This project takes all fifteen.** Not a profile, not a subset chosen
and defended: the whole language. So this document is not a claim to be
argued for. It is a list, and it has three jobs:

1. **every construct in IDL 4.2, measured** against `crates/idl-parse`
   and `crates/idl-lower` — never asserted;
2. **every construct this front end does not take yet, with what it
   costs to take it.** That list is now empty, and the section it lived
   in says what closed each row;
3. **every construct a foreign front end cannot take**, in the same
   table, because that is what a user needs before writing the
   declaration.

`union switch (octet)` is the row that started this. It is legal under
Building Block Extended Data-Types, rule (196); this project takes it;
three of the eight front ends measured here cannot compile it. That is
the row shape.

**A construct this project accepts that a peer's front end cannot compile
is an interoperability fact of the same kind as a byte difference.** It
belongs beside [`wire-feature-inventory.md`](wire-feature-inventory.md)
for that reason, and it was invisible until this document: the corpus in
`tests/interop-corpus/` exercises four of the fifteen blocks, so eleven
of them had never been measured against anything.

Sixteen defects were on this list and all sixteen are closed. What each
of them was, and what closed it, is in the three sections that held
them:

- **Constructs we did not take.** Nine rules, in
  [What was not taken](#what-was-not-taken). None had a reason that
  survived the standard, and none is refused now.
- **Constructs we took that IDL 4.2 does not have.** Four, in
  [Wider than the standard](#wider-than-the-standard). Taking everything
  in IDL 4.2 is not the same as taking things IDL 4.2 does not contain,
  and a file that uses one of these is unportable — which it is now told.
- **Three constructs that vanished** — parsed, dropped, and diagnosed by
  nobody. [Three constructs that no longer
  vanish](#three-constructs-that-no-longer-vanish). A silent drop is the
  worst of the three classes and is why that section is first.

The §9 profiles, for reference, because a peer is written against one and
not against the language: §9.2.1 Plain CORBA, §9.2.2 Minimum CORBA,
§9.2.3 CCM, §9.2.4 CCM with Generic Interaction Support, §9.3.1 Plain
DDS, §9.3.2 Extensible DDS, §9.3.3 RPC over DDS. Extensible DDS is the
one a DDS peer takes and it is Core Data Types, Extended Data-Types,
Anonymous Types and Annotations — four of the fifteen. A caller who wants
a model measured against one of them asks
`Ast::validate(Profile::…)` for it (`crates/ast/src/profile/`).

## The fifteen blocks

Rule ranges are §7.4's, taken from the item-by-item audit in
`website/spec-coverage/idl-4.2.html`. `Impl` is what
`crates/idl-parse` plus `crates/idl-lower` do with the block's own
constructs, measured 2026-08-11.

| § | building block | rules | Impl | not taken yet |
|---|---|---|---|---|
| 7.4.1 | Core Data Types | (1)–(68) | **complete** | — |
| 7.4.2 | Any | (69)–(70) | **complete** | — |
| 7.4.3 | Interfaces – Basic | (71)–(96) | **complete** | — |
| 7.4.4 | Interfaces – Full | (97) | **complete** | — |
| 7.4.5 | Value Types | (98)–(110) | **complete** | — |
| 7.4.6 | CORBA-Specific – Interfaces | (111)–(124) | **complete** | — |
| 7.4.7 | CORBA-Specific – Value Types | (125)–(132) | **complete** | — |
| 7.4.8 | Components – Basic | (133)–(143) | **complete** | — |
| 7.4.9 | Components – Homes | (144)–(152) | **complete** | — |
| 7.4.10 | CCM-Specific | (153)–(170) | **complete** | — |
| 7.4.11 | Components – Ports and Connectors | (171)–(183) | **complete** | — |
| 7.4.12 | Template Modules | (184)–(194) | **complete** | — |
| 7.4.13 | Extended Data-Types | (195)–(215) | **complete** | — |
| 7.4.14 | Anonymous Types | (216)–(217) | **complete** | — |
| 7.4.15 | Annotations | (218)–(227) | **complete** | — |

Every row was produced by running a minimal file per rule through
`zerodds_idl_parse::parse` and `zerodds_idl_lower::lower` and reading
both diagnostic lists and the resulting model. "Complete" means every
construct the block introduces parses, lowers, and leaves no broken or
pending reference behind.

**A "complete" row is worth what the measurement behind it is worth.**
7.4.15 carried one while `@::key` and `@ann::mark(7)` were refused: rule
(218) writes an annotation's name as a `<scoped_name>` and the parser
took one token, so the row rested on the half of the production that had
a test. That is the failure mode this table has, and the answer to it is
more cases rather than a different table. Fifty-four minimal files, one
per construct of every block above, now stand behind these rows;
`docs/annotation-scoped-name-followup.md` records that sweep and what it
found — 7.4.15 was the only row resting on a partial implementation.

### Three constructs that no longer vanish

`any` (§7.4.2 rules (69)(70)), `Object` (§7.4.6 rules (117)(118)) and
`ValueBase` (§7.4.7 rules (131)(132)) were the three keywords
`crates/idl-parse/src/type_spec.rs::is_primitive_keyword` admitted and
`crates/idl-lower/src/lower.rs::primitive_of` had no arm for. The member
was created, its type became `Ref::Broken`, **and neither the parser nor
the lowering emitted a diagnostic**:

```sh
# struct S { any a; };  — before
#   parse diagnostics: none
#   lower diagnostics: none
#   model:             S.a = Broken
```

The first thing that noticed was the layout planner, three layers later
and in a different vocabulary — `the layout could not be planned: type
NodeId(2) has an unresolved reference`. A front end that neither accepts
a construct nor refuses it by name is the failure class this tree treats
as the worst one, and these were three instances of it.

All three are now built-in type names of the model, created on demand in
the root module exactly as `long` is: `TypeKind::Any`,
`TypeKind::ObjectRef { interface: None }` and `TypeKind::ValueBase`.
`struct S { any a; Object o; ValueBase v; };` lowers with no broken
reference and no diagnostic, and `crates/ast`'s profile table rules on
each of them in all seven profiles.

**Where they have no wire form they are refused by name**, which is a
different statement from failing to parse:

| construct | what the layout planner says |
|---|---|
| `any` | *`any` has no wire form in DDS-XTypes; it is encoded only by CORBA CDR, which writes a TypeCode ahead of the value* |
| `Object` | *an object reference has no wire form in DDS-XTypes; CORBA writes an IOR for one, which is GIOP's framing and not XCDR's* |
| `ValueBase` | *`ValueBase` has no wire form in DDS-XTypes, which has no valuetype; CORBA CDR writes a chunked value with its repository id ahead of it* |

The C codec says the same three sentences through `EncodeError::NoWriter`,
and the C header materialiser refuses a member of such a type through
`MaterialiseError::NoLanguageForm` — it used to write `any a;` into the
struct, a declaration no compiler reads, and report success.

`TypeKind::ObjectRef` and `TypeKind::ValueBox` both existed in
`crates/ast/src/node/type_.rs` and neither was reachable from IDL. Both
are now: `Object` reaches the first and rule (126)'s `valuetype VB long;`
the second.

### What was not taken

Nine rules of IDL 4.2 this front end refused, plus the three that
vanished above. Each was reduced to the smallest file that triggered it;
none had a reason that survived the standard, and none is refused now.
There was no "outside the block we claim" column here and there will not
be one.

| rule | construct | what happened | what closed it |
|---|---|---|---|
| (43) | `const fixed C = 3.14d;` | refused, and the diagnostic named the wrong rule: *the digits and scale of `fixed` must both be integer constants* is (42)'s sentence, about a member | the constant takes its digits and scale from its value — §7.4.1.4.3 and all four rows of Table 7-11, with the 31-digit cap after each. An integer operand is refused, which is what all four ORBs that take `const fixed` do. §7.4.1.4.3's own example `0123.450d` did not lex, a leading zero having committed the lexer to octal before it looked past the digits; that is fixed with it |
| (69)(70) | `any` | vanished — see above | `TypeKind::Any`, one built-in node per model, refused at serialisation by name |
| (75) | `interface I; interface I { … };` | `DuplicateName` **and** `UndefinedForward`: the forward bound a `Structure` placeholder the definition could not complete | `Ast::add_forward_interface` and `Lowering::declare_interface`, the counterpart of `declare_type`. The header's `local` or `abstract` is carried across |
| (114) | `module m { typeprefix m "…"; };` | `UnresolvedName`: the name was qualified with the scope that wrote it and became `m::m` | a directive's target is a name being *used*, so §7.5 resolves it from that scope outwards; `typeid`, `typeprefix` and `keylist` read it through one function. A root-anchored name means the root, rule (4) |
| (126) | `valuetype VB long;` | parse error — *expected `{` opening the valuetype body* | the node's kind is decided after the name, which is where (126) and (98) diverge. `ValueBoxDecl` is its own CST kind and `TypeKind::ValueBox` is reachable |
| (127) | `abstract valuetype AV { … };` | parse error — the parser took `abstract` and committed to `interface` | one token of lookahead. Rule (105)'s forward valuetype went with it: a bodiless valuetype was declared **complete**, so its definition was a `DuplicateName` |
| (135) | `component C; component C { };` | as (75) | as (75), and `home`, `connector` and `porttype` with it |
| (188) | `module M<sequence<long> S> { … };` | parse error on the formal parameter | `sequence` appears twice in (188): the bare classifier and a whole `<sequence_type>`. Both parse, and the constrained form keeps its element in `TemplateParam::constraint` |
| (190)–(192) | `module M<long> N;` | parse error — instantiation was not parsed at all | the template's body is no longer lowered at the declaration. It is kept whole and lowered once per instantiation with each formal bound to its actual — substitution by binding, because every reference to a formal is a name and a name is what a scope answers |
| (193) | `alias M<U> R;` | parse error — a template module reference was not parsed | it hands the enclosing instantiation's bindings on unchanged, through the same `instantiate` |

The paragraph that used to stand here — a template module *declaration*
that parsed and lowered and left `S.a` `Ref::Pending` for ever, with no
diagnostic — closed with (190)–(192) rather than beside them. §7.4.12
says why: a template module's contents are definitions *of a template*,
and a member whose type is a formal parameter has no type until an
actual one is given. The body is not lowered at the declaration at all,
so there is no pending reference left to never resolve.

### Wider than the standard

Taking all fifteen blocks is not the same as taking constructs IDL 4.2
does not contain. These four were accepted and are not IDL, and a file
that uses one is unportable without being told so — which was the
defect, not the acceptance.

| what | rule it exceeds | what happened | what closed it |
|---|---|---|---|
| `keylist S (a);` | in no building block; RTI's own directive | gated by `Dialect::rti`, and the gate fired only when the dialect **refused** the construct — while `zerodds_idl_parse::parse` passes `Dialect::ALL_VENDORS` and every caller in this tree uses `parse`. The gate never fired for anybody | the fact is recorded whatever the dialect, and only the severity is the dialect's: an error when the flag is off, `Severity::Warning` when it is on. The default parse still reads vendor files and now says which vendor it admitted |
| `struct`, `union`, `bitmask`, `bitset` and `native` inside an `@annotation` body | (221) `<annotation_body>` admits `<annotation_member>`, `<enum_dcl>`, `<const_dcl>` and `<typedef_dcl>` and nothing else | accepted, no diagnostic | one match arm in `parse_annotation_decl`, naming (221). The declaration is still read after the diagnostic, so the rest of the body is not reported as a second mistake |
| `enum E { };` | (57) `<enum_dcl>` requires at least one `<enumerator>` | accepted, no diagnostic | one check per rule in the lowering — and the same shape one rule over, (52) `<switch_body> ::= <case>+`, and one building block over, (204)'s `<bit_value>`, both of which had the identical hole. The two empty bodies the grammar *does* admit, (198)'s empty `struct` and (200)'s `<bitfield>*`, stay legal |
| an identifier differing from a keyword only in case | §7.2.4 makes it illegal | **worse than accepted**: the lexer matched keywords with `eq_ignore_ascii_case`, so `CUSTOM` *became* the keyword `custom` and `enum E { AA, CUSTOM };` failed with five parse errors naming `valuetype`, never naming the rule | §7.2.4 is two sentences and they are enforced in two places. The lexer matches exactly — including the four entries Table 7-6 capitalises, `TRUE`, `FALSE`, `Object` and `ValueBase` — and the lowering reports the collision as `IDL0017`, naming the keyword and both clauses. The `_name` escape turns the check off, §7.2.3.2. See [The keyword-case rule](#the-keyword-case-rule) |

`struct C : A, B;` is correctly refused — (195) admits one
`<scoped_name>`.

## What the eight foreign front ends accept

Eight distinct IDL front ends are measured in this tree. `opendds` runs
two programs and `tao` is one of them; `ros-fastrtps` and `ros-cyclone`
share one front end. Versions are
[`tests/interop-results/SYSTEMS.md`](../tests/interop-results/SYSTEMS.md)'s.

| front end | product |
|---|---|
| `idlc` | Eclipse Cyclone DDS 11.0.1 |
| `rtiddsgen` | RTI Connext DDS 7.7.0, generator 4.7.0 |
| `fastddsgen` | eProsima Fast DDS 3.6.1 |
| `opendds_idl` | OpenDDS 3.34.0 |
| `tao_idl` | TAO 2.5.24 (OpenDDS's C++ mapping, and TAO's own column) |
| `omniidl` | omniORB 4.3.3 |
| `org.jacorb.idl.parser` | JacORB 3.9 |
| `rosidl_parser` | ROS 2 Humble, `rosidl` 3.1.6 |

Run 2026-08-11 on the Linux test host, one minimal file per block through
each front end's own command line, with no compile after it. The
invocations are `tools/interop-results/warning-sweep.sh`'s.

**Three verdicts, not two.** A front end that exits 0 and prints
*"…will be ignored"* has given a third answer, and the distinction
decides whether a construct survives to the wire:

| | meaning |
|---|---|
| **yes** | accepted, exit 0, no diagnostic |
| **drop** | accepted, exit 0, and a warning saying it is discarded |
| **no** | refused |

A **yes** is a statement about the front end and not about the generated
code. This sweep runs the front end and stops — no fill, no compile, no
link — for the reason `warning-sweep.sh` gives: it is one code
generation per case and nothing after it. Whether a construct a front
end accepted silently also reaches its output is a separate measurement.

| § | building block | `idlc` | `rtiddsgen` | `fastddsgen` | `opendds_idl` | `tao_idl` | `omniidl` | JacORB | `rosidl` |
|---|---|---|---|---|---|---|---|---|---|
| 7.4.1 | Core Data Types | partial¹ | partial¹ | partial¹ | yes | partial² | partial³ | yes | no⁴ |
| 7.4.2 | Any | no | no | yes | no | yes | yes | yes | no |
| 7.4.3 | Interfaces – Basic | no | drop | drop | yes | yes | yes | yes | no |
| 7.4.4 | Interfaces – Full | no | drop | drop | yes | yes | yes | yes | no |
| 7.4.5 | Value Types | no | yes¹⁵ | drop | yes | yes | yes | yes | no |
| 7.4.6 | CORBA-Specific – Interfaces | no | drop | no⁵ | yes | yes | no⁶ | no⁶ | no |
| 7.4.7 | CORBA-Specific – Value Types | no | drop | drop⁷ | no⁸ | no⁸ | no | no | no |
| 7.4.8 | Components – Basic | no | drop | yes | yes | no⁹ | no | no | no |
| 7.4.9 | Components – Homes | no | drop | drop | yes | no⁹ | no | no | no |
| 7.4.10 | CCM-Specific | no | drop | drop | no⁹ | no⁹ | no | no | no |
| 7.4.11 | Ports and Connectors | no | drop | yes | yes | no⁹ | no | no | no |
| 7.4.12 | Template Modules | no | drop | no | decl only | decl only | no | no | no |
| 7.4.13 | Extended Data-Types | partial¹⁰ | partial¹¹ | **yes** | partial¹² | no | no | no | partial¹⁰ |
| 7.4.14 | Anonymous Types | yes | yes | yes | yes | yes | yes | yes | yes |
| 7.4.15 | Annotations | yes | drop¹³ | yes | yes | no¹³ | no | no | partial¹⁴ |

**Fast DDS is the only one of the eight that takes Extended Data-Types
whole** — `map`, `bitset`, `bitmask`, struct inheritance, the empty
struct, all four narrow discriminators and the eight explicit-width
integers, ten cases of ten. It is also the one that refuses the most of
§7.4.6 and §7.4.7. The two facts are the same fact: the DDS front ends
implement the XTypes half and the ORBs the CORBA half, and the corpus in
`tests/interop-corpus/` lives entirely inside the first.

1. Four front ends take part of the block and not all of it. `idlc`
   refuses `long double` (*Long doubles are currently unsupported*),
   `fixed<P,S>` and `native` — rules (24), (42)/(43) and (61).
   `rtiddsgen` refuses `fixed<P,S>` — *Invalid member type* — and drops
   `native` — *native Handle will be ignored*. `fastddsgen` refuses
   `fixed<P,S>` — *Fixed type is not supported* — and drops `native`.
   `rosidl_parser` has neither, and no `union` either, which is what
   makes its cell **no** rather than partial.
2. `tao_idl -as` refuses `fixed<P,S>` as a member — *fixed data types are
   not supported*, exit 8 here — and **accepts `const fixed CC = 3.14d;`**,
   which is rule (43), the one rule of the block this project refuses.
   `tests/interop-results/SYSTEMS.md` records the same refusal reaching
   the corpus at exit 0; the exit status this front end gives a refusal
   is not stable and is not what the sweep reads.
3. `omniidl -bcxx` accepts `native Handle;` in its front end and then
   fails in its C++ backend — *omniidl: Fatal error in C++ backend*. The
   rest of the block, `fixed<P,S>` and `const fixed` included, it takes.
4. `rosidl_parser` takes a strict subset by design: modules two deep, one
   to three structures per file, no `union`, no `exception`, no
   `interface`, no `native`. It is not an IDL 4.2 front end and is
   measured as what it is. It is the reason
   `crates/wire-probe/tests/fixtures/ros_reach.md` finds a `rosidl` form
   for 123 of the corpus's 335 cases.
5. `fastddsgen`: *'Object'. Object type is not supported*.
6. `omniidl` and JacORB both refuse `typeid` / `typeprefix` — (113)(114)
   — with a syntax error, while accepting the rest of the block.
7. *WARNING: ValueType declarations are not supported. Ignoring…* — but
   `ValueBase` as a parameter type is refused outright.
8. Both OpenDDS programs: *custom is not supported* — (128).
9. `tao_idl` and `opendds_idl` parse the component grammar and then fail
   resolving `Components::Cookie`, which is the CCM support IDL neither
   ships. It is a refusal in practice and its reason is not the grammar.
10. `idlc` takes `octet`, `int8` and `uint8` discriminators, struct
    inheritance, the empty struct, `bitmask` and the eight explicit-width
    integers; it refuses `map`, `bitset` and a `wchar` discriminator.
    `rosidl_parser` takes the eight explicit-width integers, rules
    (206)–(215), and nothing else of the block.
11. `rtiddsgen` takes struct inheritance, the empty struct and all four
    narrow discriminators; it refuses `map`, `bitset`, `bitmask` and a
    `wchar` discriminator.
12. `opendds_idl` takes `map` and `int8`/`uint8` discriminators; it
    refuses `bitset`, `bitmask`, struct inheritance, the empty struct and
    `octet`/`wchar` discriminators — *union with illegal discriminator
    type*.
13. `rtiddsgen` honours the builtin annotations and drops a user
    `@annotation`: *Custom annotation Rating will be ignored*. `tao_idl`
    is measured here as `tao_idl -as`, which is the TAO column's own
    configuration and refuses the block outright — *Invalid use of this
    version of IDL*. In the **OpenDDS** column the same binary runs with
    `--idl-version 4`, parses every annotation and drops it, which is the
    144-case row of
    [`tests/interop-results/WARNINGS.md`](../tests/interop-results/WARNINGS.md)
    and inventory row **G15**. One program, two configurations, two
    different answers.
14. `rosidl_parser` honours the builtin annotations — `@final`, `@key`,
    `@optional` all parse — and has no `@annotation` declaration, rule
    (219).
15. `rtiddsgen` is the one front end that generates for a plain
    `valuetype` rather than dropping it, and it deviates while doing so:
    *value types cannot contain private state members. The private
    members will be considered public when generating code* — rule (106)
    `<state_member>`. Everything §7.4.7 adds on top it drops by name.

### What the table settles about `union switch (octet)`

`switch_type_spec` takes `<octet_type>` under **Building Block Extended
Data-Types, rule (196)** — `<switch_type_spec> ::+ <wide_char_type> |
<octet_type>`, §7.4.13.3, p. 90. It is not in Core Data Types, which is
why the three ORBs refuse it: CORBA's profiles (§9.2.1, §9.2.2) contain
Core Data Types and not Extended Data-Types.

That is the statement row **J9** of
[`wire-feature-inventory.md`](wire-feature-inventory.md) was missing. Its
reason — *"IDL 4.2 widens the integer types to include `int8` and `uint8`
and does not add `octet`"* — is wrong; rule (196) adds it explicitly.
The verdict J9 reaches is right and the reason is now the block, not the
integer list.

The same rule covers the `wchar` discriminator, which is why `rtiddsgen`
refusing `wchar` and accepting `octet` is a **partial** implementation of
one block rather than a disagreement about two constructs.

### The keyword-case rule

§7.2.3 makes an identifier that differs from a keyword only in case
illegal. It is not a building-block question and it divides the field
more sharply than any block does. `enum Ee { AA, CUSTOM, BB };`:

| front end | answer |
|---|---|
| `idlc` | accepts, silently |
| `rtiddsgen` | accepts, and names the clause: *CUSTOM identifier, collide with an idl keyword (Case-Insensitive). This is not compliant with the IDL/XTypes specification* |
| `fastddsgen` | refuses: *Illegal identifier: CUSTOM is a keyword, use escape character if you want to use it as identifier (\_CUSTOM)* |
| `opendds_idl` | refuses: *spelling differs from IDL keyword only in case: "CUSTOM"* |
| `tao_idl` | refuses, same text |
| `omniidl` | refuses: *Identifier 'CUSTOM' clashes with keyword 'custom'* |
| JacORB | refuses: *Identifier CUSTOM collides with keyword custom* |
| ZeroDDS | refuses, and names the keyword and both clauses: *`CUSTOM` differs from the keyword `custom` only in case … Write the keyword's own spelling, or escape the identifier as `_CUSTOM`* — `IDL0017` |

ZeroDDS used to **mis-lex** it: `CUSTOM` became the keyword `custom` and
the file failed with five parse errors naming `valuetype`. §7.2.4, p. 20
is two sentences — "Keywords must be written exactly as shown in the
above list. Identifiers that collide with keywords (see 7.2.3,
Identifiers) are illegal" — and one `eq_ignore_ascii_case` was applying
the first as though it were the second. Recognition is exact now and the
collision is §7.2.3's, reported one layer up.

Table 7-6 capitalises four of its entries and this table now spells them
as it does: `TRUE`, `FALSE`, `Object` and `ValueBase`. Measured on the
Linux test host, `interface I { void f(in object o); };` — OpenDDS and
TAO answer *identifier spellings differ only in case: "object" and
"Object"*, JacORB *Identifier object collides with keyword Object*,
omniORB *not found*. `true` for `TRUE` gets the same three answers.

One IDL file in this tree is on the wrong side of the clause:
`crates/spatial-dds/idl/spatial_core.idl:664` is `@value(255) CUSTOM`.
Five of seven IDL front ends refuse the file for it, this one now refuses
it by name, and nothing compiles it — the crate is tracked and is not a
workspace member. This state was recorded in the historical internal
backlog under "What the demolition left behind". That maintainer record
is not public release evidence or a current RC7 status statement.

### The case-collision rule, §7.2.3.1

`crates/idl-lower` enforces it — `Lowering::one_namespace_per_scope`,
`IDL0008`. The field is split, and how it is split decides what a fix
can be. `struct Roi { … }; struct Rq { Roi roi; };` in one module:

| spelling | `idlc` | `rtiddsgen` | `fastddsgen` | `opendds_idl` | `tao_idl` | `omniidl` | JacORB | ZeroDDS |
|---|---|---|---|---|---|---|---|---|
| `ROI roi;` — the member's type is in the same module | yes | yes | **no** | no | no | no | yes | no |
| `sp::common::ROI roi;` — the same, qualified | yes | yes | **no** | yes | yes | yes | yes | yes |
| `Health health;` where `Health` is an **enum** in the same module | yes | yes | yes | no | no | no | yes | no |
| `sp::common::Health health;` | yes | yes | yes | yes | yes | yes | yes | yes |
| `GeoPose geopose;` via a local `typedef` of a type in another module | yes | yes | yes | no | no | no | yes | no |
| `sp::core::GeoPose geopose;` — the typedef deleted | yes | yes | yes | yes | yes | yes | yes | yes |
| the member renamed | yes | yes | yes | yes | yes | yes | yes | yes |

Three different rules are being applied:

- **OpenDDS, TAO and omniORB** compare the member's name against the
  simple type name *written beside it*. Qualifying the type silences
  them, which is the same rule `crates/idl-lower` implements
  (`collect_simple_type_names` takes no part in a qualified name).
- **Fast DDS** compares the member's name against every identifier
  *defined* in the enclosing module, case-insensitively, and it registers
  **struct** names there and not enumeration names. No spelling of the
  type reaches it: only renaming the member does.
- **Cyclone, RTI and JacORB** do not apply it at all.

One more measurement belongs here and it is not about the collision at
all. **`fastddsgen` refuses every root-anchored scoped name**, whether or
not anything collides:

```text
::mm::GeoPose geopose;      fastddsgen: '::' was not defined previously
::aa::bb::GeoPose geopose;  the same
::GeoPose geopose;          the same
::mm::GeoPose from nn       the same
```

Four shapes measured, four refusals. `<scoped_name>` rule (4) admits
`"::" <identifier>` and Fast DDS 3.6.1 does not implement that
alternative. The module-relative form without the leading `::` is the
one seven of seven accept, so it is the form to write and the leading
`::` is the form to avoid in anything a Fast DDS peer will read.

## Reproducing this

The ZeroDDS column is one minimal file per rule through the two front-end
crates. The foreign columns are the same files through each vendor's own
command line, on the Linux test host, with no compile after:

```sh
# one construct, this project's front end
cargo run -q -p zerodds-wire-probe --bin wireprobe-zerodds -- \
    --idl case.idl --type S --values values.json --encoding xcdr2

# the same file, each foreign front end — the invocations are
# tools/interop-results/warning-sweep.sh's `run_front_end`
"$CYCLONE_HOME/bin/idlc" case.idl
"$NDDSHOME/bin/rtiddsgen" -language C -platform "$RTI_TARGET" -replace case.idl
"$FASTDDSGEN" -t . -replace case.idl
"$OPENDDS_IDL" -I"$OPENDDS_HOME/include" --idl-version 4 case.idl
"$TAO_IDL" -as -I"$OPENDDS_HOME/include" -o . case.idl
omniidl -bcxx -Wbh=.hh -Wbs=SK.cc case.idl
"$JACORB_HOME/bin/idl" -d . case.idl
python3 -c 'from rosidl_parser.parser import parse_idl_string; parse_idl_string(open("case.idl").read())'
```

`tools/interop-vendors.sh` is where each toolchain is looked for and what
to set when it lives somewhere else. A missing toolchain is reported by
name and is never a silent skip.
