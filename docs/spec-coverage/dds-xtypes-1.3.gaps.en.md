# OMG DDS-XTypes 1.3 — the negative list

**Spec:** [OMG DDS-XTypes 1.3](https://www.omg.org/spec/DDS-XTypes/1.3/),
formal/2020-02-04, 340 pages. The PDF sits in the local cache at
`internal/standards/cache/omg/dds-xtypes-1.3.pdf` (gitignored).

**Tool:** `tools/xtypes-1.3-gap-scan/scan.sh`. **Denominator:**
`tools/xtypes-1.3-gap-scan/roster.py` and the `roster.tsv` it writes,
plus `anchors.tsv`. **Evidence:**
`tools/xtypes-1.3-gap-scan/cases.tsv` (199 probes) and `expected.tsv`
(the recorded state).

The other pages under `docs/spec-coverage/` are positive lists: they name
what was looked at and how far it got. What was never looked at cannot
appear in them, because their content is derived from the tree and not
from the specification. This page is the other direction.

Every line here has an attempt behind it: the octets or the IDL that were
fed in, and what came back. No line rests on a guess.

## The denominator, and why it costs more here

OMG IDL 4.2 has Annex A: a consolidated, numbered grammar from which a
complete inventory falls out with a hole check (`range(1, 228)` against
the parsed numbers). **XTypes 1.3 has no grammar.** Its inventory is a
set of normative tables and IDL blocks, and each needs its own extractor
and its own completeness argument. `roster.py` pulls seven axes out of
the PDF and prints the check for each:

| Axis | Denominator | Completeness check |
|---|---:|---|
| `typekind` | 28 | Annex B declares the TypeKind constants **twice** — once as `const octet TK_x`, once as `const TypeKind TK_x`. Both listings must agree on every name and every value, and no value may appear twice. |
| `typeid` | 31 | Every `TI_*` constant Annex B declares must be a `case` of `union TypeIdentifier`, and every `case` label a declared constant. Plus the rows of Table 29. |
| `annotation` | 24 | §7.3.1.2.1 has sixteen numbered subclauses; hole check `range(1, 17)`. Plus the 19 annotations whose applicability Table 21 restricts. |
| `assign` | 55 | §7.2.4.4 has eight subclauses (hole check `range(1, 9)`), Tables 14 through 19 carry the definitions. The 55 rules live in `anchors.tsv`, **each with a literal fragment of the PDF**, without which `roster.py` fails. |
| `typeobject` | 13 | `MinimalTypeObject` and `CompleteTypeObject` are two unions over the same discriminator; the check is that their `case` sets are equal. `TypeObject` itself carries exactly `EK_COMPLETE` and `EK_MINIMAL`. |
| `typelookup` | 16 | §7.6.3.3 has four subclauses (hole check `range(1, 5)`), §7.6.3.3.4 exactly two operations, §7.6.3.3.3 ten `TypeLookup_*` types. |
| `datarep` | 16 | Table 59 carries four verdicts, Table 60 twelve rows of representation, extensibility, encoding version, endianness and identifier value. All twelve must parse. |

**183 items over seven axes, 19 checks, all holding.**

What that proves and what it does not: that every rule listed here is in
the specification is checked. That these are *all* the rules of the named
clauses is the claim of the person who read them, not a check. For
`typekind`, `typeid`, `typeobject` and `typelookup` the denominator falls
out of the IDL and is therefore machine-complete; for `assign`,
`annotation` and `datarep` it is assembled by hand and only backed item
by item.

## How it was measured

The probes here are octets, not text. Three measurements per probe; what
they mean depends on the op, and the table is in the header of
`tools/xtypes-1.3-gap-scan/probe/src/main.rs`:

| Op | `decode` | `encode` | `rule` |
|---|---|---|---|
| `ti` | the octets read as a `TypeIdentifier` | re-encoding them gives the same octets back | §7.2.4.1: the identifier is assignable from itself |
| `to` | the octets read as a `TypeObject` | the same | §7.6.3: the object becomes a model |
| `todh` | the type builds from IDL | the encoded `TypeObject` opens with a DHEADER covering the rest (§7.4.2 for `@extensibility(APPENDABLE)`) | — |
| `idl` | the IDL parses, lowers and resolves | the `MinimalTypeObject` survives encode, decode and encode byte for byte | the `CompleteTypeObject` builds |
| `assign` | both types build from one IDL text | — | §7.2.4.4: `is_assignable(T2, T1)` said yes |
| `datarep` | — | — | §7.6.3.1 Table 59: the negotiation accepted |
| `encaps` | — | — | §7.6.3.1.2 Table 60: the pairing is in the matrix |
| `tl` | the octets read as the named TypeLookup type | re-encoding gives the same octets back | — |

Plus one measurement that is not about the code: `corpus` counts how many
of the tree's 10,722 committed sources contain the probe's string at all
(the `witness` column of `cases.tsv`). `*.md` is deliberately not in the
corpus — a page that names a construct is not evidence that anything
exercises it, and this page would otherwise be counting itself. A zero is
a fact: nothing in the tree names the construct, so nothing in the tree
can have exercised it. A `-` means no string isolates the construct.

## The number

The survey found **199 probes, 166 with no finding**. The remaining 33
fell out like this:

| | Category | Probes | of those closed |
|---|---|---:|---:|
| A | not recognised — the tree refuses what the specification allows | 17 | 16 |
| B | accepted although the specification says otherwise | 9 | 7 |
| C | recognised, but not emitted that way | 6 | 5 |
| D | emitted, and no source in the tree names it | 1 | 1 |
| E | deliberate, with a reason | 3 | — |

Category B is the one the predecessor added for OMG IDL 4.2, and it is
the important one here too: there is no error message, only a model or a
verdict with the wrong content. Six of the nine B lines are assignability
rules that pass silently — two endpoints match that §7.2.4.4 says must
not.

## What came of it

**Thirty of the 33 are closed. Three stay open, because they are not
gaps in this tree.**

Every closed line below carries a `**Closed:**` paragraph naming the
test that holds it. The three others — A.8, B.8/B.9 and C.4 — are places
where the specification contradicts itself: the question there is not
whether the tree is wrong but which of the two sentences it follows, and
that is a decision rather than a repair. They are written up as **B7**,
**B8** and **B9** in
[`docs/omg-xtypes-issues.md`](../omg-xtypes-issues.md), in the shape the
OMG issue form takes.

The probe count went from 199 to **201**: two cases were added that tell
the new framing from the old (`tl-getTypes_in-extra-member`,
`tl-call-unknown-operation`). Eleven probes got a new `arg` — the `tl-*`
and `to-*` octets had been written against the tree's form rather than
the clause's, and the head of `cases.tsv` says such a probe measures
nothing.

**The vendor comparison this page recorded as missing has been run.** On
2026-09-06 on the Linux test host, with the toolchains installed there:
`idlc` (CycloneDDS) and `rtiddsgen` +
`DDS_TypeObjectV2_create_from_typecode` (RTI Connext 7.7.0) over the
same IDL. For `Plain` COMPLETE both report the hash
`7c0c6fab7d066a0c56b8e93c0ea7`; Cyclone's stream is 80 octets and opens
`4c000000` — a DHEADER of 76 — and RTI's blob is 104 octets and holds
exactly those 80 at offset 24. ZeroDDS produces them byte for byte.
`crates/types/tests/typeobject_vendor_dheader.rs` holds all four cases
in both forms; `tools/typeobject-bytes/record.sh` re-derives them from
the vendors.

---

## A — not recognised

The specification allows or requires it; the tree refuses it.

### A.1 `TK_ANNOTATION` has no TypeObject in this tree

**Spec:** Annex B, `union MinimalTypeObject switch (octet)` —
`case TK_ANNOTATION: MinimalAnnotationType annotation_type;`. Table 29
additionally lists `TK_ANNOTATION` as a TypeIdentifier discriminator.

**Fed:** `@annotation A { long v; };`

**Result:** ``no type named `A` `` — the declaration parses and lowers,
but the model then carries no type `A`, so `from_ast` can never build a
`MinimalAnnotationType`. The variant exists
(`crates/types/src/type_object/minimal/annotation_type.rs`); it is never
produced from IDL.

**Corpus:** 4 of 10,722.

**Probe:** `tk-annotation`

**Closed.** `from_ast::annotation_type_object` builds both forms, and
`Ast::annotation_defs` is the counterpart to `Ast::types` for the other
declaration kind that has a TypeObject. No `TypeNode` is created for it:
`@Name` is meant to resolve to the declaration and not to a type. Test:
`crates/types/tests/type_object_annotations.rs::an_annotation_declaration_has_a_type_object`.

### A.2 Six of the ten TypeLookup types of §7.6.3.3.3 do not exist

**Spec:** §7.6.3.3.3 declares ten `TypeLookup_*` types: the four
`_In`/`_Out` structures, the two `_Result` unions, `TypeLookup_Call`,
`TypeLookup_Return`, and the two RPC envelopes `TypeLookup_Request`
(`@RPCRequestType`) and `TypeLookup_Reply` (`@RPCReplyType`).

**Result:** `crates/types/src/type_lookup.rs` has four counterparts —
`GetTypesRequest`, `GetTypesReply`, `GetTypeDependenciesRequest`,
`GetTypeDependenciesReply`. The other six are missing.
`TypeLookup_Call`, `_Return`, `_Request` and `_Reply` are the messages in
which the four travel over the built-in endpoints of Table 61; without
them there is no call, only the payload of one.

**Corpus:** 0 of 10,722 for each of the six names.

**Closed.** All ten types are in `crates/types/src/type_lookup.rs`,
each with the framing its extensibility gives it: the four parameter
types `MUTABLE`, the other six appendable, because §7.3.1.2.1.8 says a
type whose representation does not indicate its extensibility kind is
appendable. `RequestHeader` is declared there rather than taken from
`zerodds-rpc`, because that crate depends on this one. Tests:
`type_lookup_call_roundtrips_both_operations`,
`type_lookup_return_roundtrips_both_operations`,
`type_lookup_request_and_reply_roundtrip`,
`get_types_result_roundtrips_both_arms`,
`get_type_dependencies_result_roundtrips_both_arms`.

**Probes:** `tl-gettypes_result`, `tl-gettypedependencies_result`,
`tl-call`, `tl-return`, `tl-request`, `tl-reply`

### A.3 The `default:` arm of `MinimalTypeObject` is refused

**Spec:** Annex B — `default: MinimalExtendedType extended_type;`, under
the comment `// =========== Future extensibility ============`.

**Fed:** `F1 C0 00000000` — EK_MINIMAL, then a TypeKind this revision
does not know.

**Result:** `UnknownTypeKind { kind: 192 }`.

**Probe:** `to-extended-arm`

**Closed.** `MinimalTypeObject` and `CompleteTypeObject` have an
`Extended` variant holding the TypeKind and the member list verbatim —
the DHEADER excluded, because that is a length the encoder writes and a
byte order the value does not carry. Tests:
`a_type_kind_this_revision_does_not_define_survives_the_round_trip`,
`an_extended_type_kind_keeps_the_member_list_it_arrived_with`.

### A.4 `@non_serialized` members count in the compatibility check

**Spec:** §7.2.4.4.2 — "Members that are marked as non-serialized, see
Sub Clause 7.3.1.2.1.14, shall be ignored during type compatibility
checking."

**Fed:** reader `@final struct R { long a; };`, writer
`@final struct W { long a; @non_serialized long b; };`

**Result:** `No("final struct member count mismatch")`.

**Corpus:** 8 of 10,722.

**Probes:** `as-nonser`, `an-non-serialized-effect`

**Closed.** The member is no longer in the TypeObject at all, which is
both halves at once: it is not on the wire, and the rules that read the
TypeObject and nothing else cannot see it. The member *index* still
advances, because a `@non_serialized` declaration occupies a position.
Test:
`crates/types/tests/table19_key_members.rs::a_non_serialized_member_does_not_enter_the_comparison`.

### A.5 `@ignore_literal_names` has no effect

**Spec:** Table 18, row `ENUMERATION_TYPE` — "Any literals that have the
same name in T1 and T2 also have the same value, and any literals that
have the same value in T1 and T2 also have the same name. **This
behavior may be modified with the `@ignore_literal_names` annotation**,
see 7.3.1.2.1.11."

**Fed:** `@ignore_literal_names @appendable enum R { A, B };` against
`@ignore_literal_names @appendable enum W { X, Y };` — same values,
different names.

**Result:** `No("an enum literal name and its value do not agree across
the two types")`. The front end takes the annotation and it changes
nothing about the verdict.

**Corpus:** 4 of 10,722.

**Probes:** `as-enum-ignore-names`, `an-ignore-literal-names-effect`

**Closed**, with a named side effect. The annotation now reaches
`EnumTypeFlag::IGNORE_LITERAL_NAMES`, and the comparison requires it on
**both** sides, as §7.3.1.2.1.11 says ("in both enumerated types"); it
read `||`, which let one side alone decide about the other's names. That
writes one bit of a flag word Annex B calls `// Unused` — the
extensibility bits of the same word are still **not** written, see
B.8/B.9 and `omg-xtypes-issues.md` §B8. Test:
`crates/types/tests/table19_key_members.rs::ignore_literal_names_needs_the_annotation_on_both_types`.

### A.6 Table 18 puts no condition on the bitmask flags; the tree does

**Spec:** Table 18 — "BITMASK_TYPE if and only if T1.bound == T2.bound";
the "Object construction" column says "Preserve bit values by index **for
all bits identified in both T1 and T2**", so it expressly allows for bits
only one side knows.

**Fed:** two bitmasks with `@bit_bound(8)` whose flags sit at different
positions; and one where the writer has an extra flag.

**Result:** both times `No("bitmask writer flag unknown in reader")`.

**Probes:** `as-bm-same-bound`, `as-bm-writer-extra-flag`

**Closed.** The flag check is gone; the bound stays, because it is the
wire width. The "Object construction" column is the argument for it and
not merely the absence of a condition: "for all bits identified in both
T1 and T2" expects bits only one side names, and says what happens to
them. Test:
`crates/types/tests/table19_key_members.rs::two_bitmasks_of_the_same_bound_are_assignable_whatever_their_flags`.

### A.7 The empty DataRepresentation list

**Spec:** §7.6.3.1 — "The default value of the
DataRepresentationQosPolicy shall be an empty list of preferences. An
empty list of preferences shall be taken to be equivalent to a list
containing the single element XCDR."

**Fed:** once an empty offer against `[XCDR]`, once `[XCDR]` against an
empty request.

**Result:** both times `NoOverlap`. `negotiate_representation` does not
apply the default; two endpoints with default QoS do not meet through
this function.

**Probes:** `dr-empty-offer`, `dr-empty-request`

**Closed.** `negotiate_representation` substitutes the single-element
list `[XCDR]` for an empty one, on both sides. The substitution is
`XCDR` and not "anything": an empty offer against a reader that takes
only XCDR2 still finds nothing. Test:
`crates/types/src/qos.rs::an_empty_list_of_preferences_is_the_single_element_xcdr`.

### A.8 XCDR1 with `APPENDABLE` is refused, and Table 60 carries it

**Spec:** Table 60, third and fourth rows —
`XCDR | APPENDABLE | 1 | Big Endian | CDR_BE | {0x00, 0x00}` and
`XCDR | APPENDABLE | 1 | Little Endian | CDR_LE | {0x00, 0x01}`.

**Fed:** `check_data_repr_extensibility(Xcdr1, Appendable)`.

**Result:** ``Err("Tab.59 §7.6.3.1: XCDR1 does not support APPENDABLE
directly — the encoder must choose FINAL")``. The refusal is deliberate
and carries its reason in the source; the place it cites is Table 59,
which says nothing about this. Table 60 says the opposite.

**Probe:** `enc-xcdr-appendable`

**Open, and not as a gap in this tree.** The contradiction is written up
as **B7** in [`docs/omg-xtypes-issues.md`](../omg-xtypes-issues.md). The
refusal stays as it is: changing it would be to pick one of the two
tables, and picking is the RTF's to do.

---

## B — accepted although the specification says otherwise

Here there is no error message, only a model or a verdict with the wrong
content.

### B.1 `TK_NONE` is a valid type and is assignable from itself

**Spec:** Table 29, first row — `TK_NONE | N/A | Invalid identifier`.

**Fed:** the octet `00`.

**Result:** `decode ok`, `encode ok`, and `is_assignable(None, None)` is
`Yes` — the identity short-circuit of §7.2.4.1 fires before anything
checks whether the identifier is valid at all. Two endpoints both
carrying the invalid identifier match.

**Corpus:** 58 of 10,722.

**Probe:** `ti-prim-none`

**Closed** in the `rule` column, and only there. `is_assignable` refuses
`TK_NONE` on either side, ahead of the identity shortcut. Decoding and
encoding stay allowed: `TypeInformation` writes `TK_NONE` for an
equivalence form the sender left out (§7.6.3.2.1), and a stream carrying
that is not malformed. Test:
`crates/types/tests/table19_key_members.rs::tk_none_is_not_assignable_from_itself`.

### B.2 `EK_BOTH` passes as a TypeIdentifier discriminator

**Spec:** Annex B — `union TypeIdentifier` has `case EK_COMPLETE:` and
`case EK_MINIMAL:` and no case for `EK_BOTH = 0xF3`. `EK_BOTH` belongs in
the `PlainCollectionHeader`, not here.

**Fed:** `F3` and fourteen octets of hash.

**Result:** accepted as `Unknown(0xF3)`; re-encoding leaves one octet,
the fourteen hash octets are gone.

**Corpus:** 5 of 10,722.

**Probe:** `ti-ek-both`

**Closed.** `EK_BOTH` at the TypeIdentifier discriminator is refused. In
a `PlainCollectionHeader`, where it belongs, nothing changes — that is
an octet field and not a union discriminator. Test:
`ek_both_is_not_a_type_identifier_discriminator`.

### B.3 The tree's SCC form leaves out two fields, and does not read them either

**Spec:** Annex B —

```idl
@extensibility(APPENDABLE) @nested
struct StronglyConnectedComponentId {
    TypeObjectHashId sc_component_id;
    long             scc_length;
    long             scc_index;
};
```

`TypeObjectHashId` is itself a union with the discriminator
`EK_COMPLETE`/`EK_MINIMAL` in front of the hash, and the file's own
header says "The types in this file shall be serialized with XCDR
encoding version 2" — an `APPENDABLE` object carries a DHEADER under
version 2 (§7.4.2).

**Fed:** the tree's form, `B0` followed directly by fourteen hash octets,
with no DHEADER and no `TypeObjectHashId` discriminator.

**Result:** `decode ok`, `encode ok` — the stream no conforming reader
should accept goes through and comes back unchanged. Consequence:
`StronglyConnectedComponentId` in this tree has no field for the
equivalence kind, and §7.3.4.6 distinguishes on exactly that ("those with
discriminator `TI_STRONG_COMPONENT` where the contained
`TypeObjectHashId` has discriminator `EK_MINIMAL`"). Minimal and complete
SCC identifiers are indistinguishable here.

**Corpus:** 5 of 10,722.

**Probe:** `ti-scc-tree-form`; the other direction is C.5.

**Closed.** `StronglyConnectedComponentId` now has the field
`equivalence_kind` and the appendable framing; the tree's 24-octet form
is refused. Tests:
`a_component_identifier_is_the_thirty_two_octets_of_annex_b`,
`a_minimal_and_a_complete_component_identifier_differ`,
`a_component_identifier_takes_only_the_two_equivalence_kinds`.

### B.4 The `@key` on a union discriminator never reaches the TypeObject

**Spec:** Table 19, row `UNION_TYPE`, third bullet — "Either the
discriminators of both T1 and T2 are keys or neither are keys."

**Fed:** `@final union R switch (@key long) { case 1: long a; };` against
`@final union W switch (long) { case 1: long a; };`

**Result:** `Yes`. `crates/types/src/from_ast.rs` sets
`discriminator_flags` to a constant
`UnionDiscriminatorFlag::IS_MUST_UNDERSTAND | try_construct_bits(…)`;
`IS_KEY` never enters it, so the rule has nothing to compare.

**Corpus:** 192 of 10,722.

**Closed.** The front end carries `@key` on the discriminator into
`XTypesFacet::discriminator_is_key`, `from_ast` into
`UnionDiscriminatorFlag::IS_KEY`, and `check_unions` compares the two.
Which reading applies is an axis rather than this front end's decision:
`KeyOnUnionDiscriminator::Refused` is the default and reports the
annotation — the measured refusal by Cyclone, RTI and Fast DDS
(2026-08-10, `/root/n11-keyunion/`) is the reason — and `Carried` is
what asks for it, all the way to a KeyHash over the discriminator alone
(`keyhash::union_key_discriminator`). Tests:
`crates/types/tests/table19_key_members.rs::one_keyed_union_discriminator_and_one_unkeyed_are_not_assignable`,
`crates/idl-lower/tests/annotations.rs::a_key_on_a_union_discriminator_is_refused_with_the_clause_that_permits_it`
and `…::a_key_on_a_union_discriminator_reaches_the_facet_when_carried`.

**Probe:** `as-union-disc-key`; the counter-probe with `@key` on both
sides (`as-union-disc-key-both`) also yields `Yes`, as it should.

### B.5–B.7 Three of Table 19's key rules do not fire

**Spec:** Table 19, row `STRUCTURE_TYPE`. The rules are there verbatim,
and the table closes with "Note: The rules regarding key members ensure
that the key of T2 can be transformed faithfully into the key of T1
without aliasing or loss of information."

| Rule (quoted) | Fed (reader against writer) | Result | Probe |
|---|---|---|---|
| "For any string key member m2 in T2, the m1 member of T1 with the same member ID verifies m1.type.length >= m2.type.length." | `@key string<8>` against `@key string<16>` | `Yes` | `as-struct-key-string` |
| "For any enumerated key member m2 in T2, the m1 member of T1 with the same member ID verifies that all literals in m2.type appear as literals in m1.type." | `@key E1` with `{ A }` against `@key E2` with `{ A, B }` | `Yes` | `as-struct-key-enum` |
| "For any sequence or map key member m2 in T2, the m1 member of T1 with the same member ID verifies m1.type.length >= m2.type.length." | `@key sequence<long,4>` against `@key sequence<long,8>` | `Yes` | `as-struct-key-seqmap` |

All three are cases where the writer's key does not fit into the reader's
key and the endpoints match anyway.

**Closed.** `key_members_fit` in `assignability.rs` checks the three
rules, and only for members carrying `IS_KEY`: a `string<8>` reader *is*
assignable from a `string<16>` writer as long as the member is not a
key — then `@try_construct` decides, and it is being a key that makes
the loss inadmissible. The other direction is checked too. Tests in
`crates/types/tests/table19_key_members.rs`:
`a_shorter_string_key_in_the_reader_is_not_assignable`,
`a_writer_enum_key_literal_the_reader_lacks_is_not_assignable`,
`a_shorter_sequence_key_in_the_reader_is_not_assignable`,
`a_shorter_string_member_that_is_not_a_key_is_still_assignable`.

### B.8/B.9 The extensibility of an enumeration is not compared

**Spec:** Table 18, row `ENUMERATION_TYPE`, first bullet —
"T1.extensibility == T2.extensibility", and third bullet — "If
extensibility is final T1 and T2 have the same literals."

**Fed:** `@final enum R { A, B };` against
`@appendable enum W { A, B };`, and `@final enum R { A, B };` against
`@final enum W { A, B, C };`

**Result:** both times `Yes` when the survey was taken; since the vendor
measurement in E.3 both times `No`.

**Why:** the same specification declares in Annex B
`typedef TypeFlag EnumTypeFlag; // Unused. No flags apply`, and
`from_ast.rs` accordingly wrote `EnumTypeFlag(0)`. RTI, OpenDDS and
Cyclone write the `TypeFlag` extensibility position into the word, and
RTI recomputes the hash of a received enumeration from its own
rendering; `from_ast.rs` now writes the bits the same way and
`check_enumerations` compares them once both sides carry them. See E.3.

**Probes:** `as-enum-ext`, `as-enum-final`

**Open, and not as a gap in this tree.** The contradiction is written up
as **B8** in [`docs/omg-xtypes-issues.md`](../omg-xtypes-issues.md). One
half of it has since been decided and is named there: the one bit
§7.3.1.2.1.11 needs for `@ignore_literal_names` is written (A.5); the
extensibility bits are not.

---

## C — recognised, but not emitted that way

The tree reads the value and writes something else back.

### C.1/C.2 The outer `TypeObject` gets no DHEADER

**Spec:** Annex B —

```idl
@extensibility(APPENDABLE) @nested
union TypeObject switch (octet) { // EquivalenceKind
case EK_COMPLETE: CompleteTypeObject complete;
case EK_MINIMAL:  MinimalTypeObject  minimal;
};
```

and, at the head of the same file, "The types in this file shall be
serialized with XCDR encoding version 2". §7.4.2 gives an `APPENDABLE`
object under encoding version 2 a DHEADER — a `UInt32` holding the length
of everything after it.

**Fed:** `@final struct S { long a; };` and
`@mutable struct S { @id(1) long a; };`, each built as a
`MinimalTypeObject` and encoded.

**Result:** 35 octets, whose first four read 86,513 and 283,121
respectively, with 31 octets after them. There is no DHEADER; the first
octet is the EquivalenceKind. The *inner* `APPENDABLE` structures do get
theirs — `framing::encode_appendable` has 19 call sites in
`crates/types/src/type_object/` — the outer union does not.

**Not measured:** whether a vendor writes a DHEADER in that position.
This tree exchanges `TypeObject` octets with no vendor (discovery matches
on `type_name`, see the "Wiring status" section of
`crates/types/src/lib.rs`), so there is no recorded vendor byte in the
tree to compare against. The next step for this finding is a
`PID_TYPE_OBJECT` from Fast DDS or Cyclone on the Linux test host.

**Probes:** `to-dheader-minimal-struct`, `to-dheader-complete-struct`

**Closed, and measured.** `TypeObject::encode_into` writes the DHEADER;
`hash::minimal_hash_input` calls that very encoder and adds nothing.
One path, not two — a second DHEADER would move every octet of every
hash, and a peer that disagrees about a hash looks like a type mismatch
rather than a framing defect.

The vendor comparison this section named as the next step has been run:
see "What came of it" above. CycloneDDS's `idlc` and RTI Connext 7.7.0
on the test host, the same octets, the same hash.

The counter-check for the discovery rejection this finding carries with
it: the announced size is the length of the stream the hash is taken
over, and that is the number RTI counts for the same object — the
rejection was a `serializedSize mismatch` of four octets. Tests:
`crates/types/tests/typeobject_vendor_dheader.rs` (four cases, both
forms, plus the hash and size equality) and
`crates/discovery/src/type_lookup/server.rs::a_dependency_is_announced_at_its_hashed_length`.

### C.3 `TI_EXTENDED` swallows its body

**Spec:** Annex B — `default: ExtendedTypeDefn extended_defn;`, and
`ExtendedTypeDefn` is `@extensibility(MUTABLE)`, so under encoding
version 2 it carries a framing of its own that a reader can skip.

**Fed:** `C0` and eight zero octets.

**Result:** decodes to `Unknown(0xC0)`, re-encodes as **one** octet. The
reader read not a single octet of the body. In a nested position — the
element type of a plain sequence, say — that shifts every following field
of the stream, and no error appears anywhere.

**Corpus:** 3 of 10,722.

**Probe:** `ti-extended`

**Closed.** `TypeIdentifier::Extended` holds the discriminator and the
member list; the framing is read and written. Tests:
`the_extended_arm_carries_its_body`,
`an_extended_arm_with_a_member_list_round_trips_verbatim`,
`the_extended_arm_encodes_its_dheader_too`.

### C.4 `TK_ANNOTATION` as a TypeIdentifier loses its hash

**Spec:** Table 29, second to last row — `TK_ANNOTATION | Annotation
Declaration | Not plain type. Has TypeObject. Uses Hash of the TypeObject
representation of the Annotation declaration`. The `union TypeIdentifier`
of Annex B has **no** case for `TK_ANNOTATION`; `roster.py` holds that
contradiction as a check so that it cannot quietly disappear.

**Fed:** `50` and fourteen octets of hash.

**Result:** decodes to `Unknown(0x50)`, re-encodes as one octet.

**Corpus:** 3 of 10,722.

**Probe:** `ti-annotation-hash`

**Open, and not as a gap in this tree.** The contradiction is written up
as **B9** in [`docs/omg-xtypes-issues.md`](../omg-xtypes-issues.md). The
behaviour changed all the same, and for the better: with the `default:`
arm framed (C.3), `50` followed by fourteen hash octets is **refused**
rather than silently shortened to one. The tree follows Annex B, out
loud. Which of the two sentences is the right one is not decided by the
page in `omg-xtypes-issues.md` — it is reported there.

### C.5 The specification's SCC form comes back shortened

**Fed:** the Annex B form — `B0`, three padding octets, a DHEADER of 24,
then `F1` as the `TypeObjectHashId` discriminator, fourteen hash octets,
one padding octet, `scc_length` and `scc_index`. 32 octets.

**Result:** `decode ok`, and **24** octets come back. The reader took the
DHEADER for the first octets of the hash. This is B.3 from the other
side.

**Corpus:** 5 of 10,722.

**Probe:** `ti-scc-spec-form`

**Closed** with B.3; see there.

### C.6 `TypeLookup_getTypes_Out` has only the first of its two fields

**Spec:** §7.6.3.3.3 —

```idl
@extensibility(MUTABLE)
struct TypeLookup_getTypes_Out {
    @hashid sequence<TypeIdentifierTypeObjectPair> types;
    @hashid sequence<TypeIdentifierPair>           complete_to_minimal;
};
```

**Result:** `GetTypesReply` in this tree has `types: Vec<ReplyTypeObject>`
and nothing else. That is neither `TypeIdentifierTypeObjectPair` — the
reply carries the TypeObjects without the identifier that was asked for —
nor is there a `complete_to_minimal`. Eight zero octets (two empty
sequences) in, four out.

**And the framing.** All six request/reply structures of §7.6.3.3.3 are
`@extensibility(MUTABLE)` and carry their members with `@hashid`; under
encoding version 2 that means a DHEADER and an EMHEADER per member. The
tree writes a bare sequence. The probe `tl-mutable-framing` feeds
`04000000 00000000` — a DHEADER of 4 and then an empty sequence — to
`GetTypesRequest`: the DHEADER is read as an element count, **four
`TK_NONE` identifiers** come out, and the stream re-encodes byte for
byte. No error, anywhere.

**Corpus:** 7 of 10,722.

**Probes:** `tl-gettypes-out`, `tl-mutable-framing`

**Closed.** `GetTypesReply` carries both members, `types` as
`sequence<TypeIdentifierTypeObjectPair>`; the four parameter types have
the MUTABLE framing with the `@hashid` member ids of §7.3.1.2.1.1. The
derivation has a check with a printed answer: the clause publishes
`TypeLookup_getTypes_HashId = 0x018252d3` and
`TypeLookup_getDependencies_HashId = 0x05aafb31`, and the same function
reproduces both. `04000000 00000000` is now refused. Tests:
`a_dheader_of_four_is_not_a_sequence_length_of_four`,
`an_empty_get_types_request_carries_its_member_header`,
`get_types_reply_carries_types_and_complete_to_minimal`,
`member_ids_are_the_hashid_of_their_name`,
`the_operation_hashes_are_the_ones_the_clause_prints`.

---

## D — emitted, and no source in the tree names it

### D.1 `@default_nested`

**Spec:** §7.3.1.2.1.7 — `@annotation default_nested { boolean value
default TRUE; };`, applied to a module declaration.

**Fed:** `@default_nested(TRUE) module M { @final struct S { long a; };
};` — it passes the front end, the `MinimalTypeObject` and the
`CompleteTypeObject`.

**Corpus:** 0 of 10,722. No `.rs`, no `.idl`, no `.tsv` in the tree
contains the string `@default_nested`.

**Probe:** `an-default_nested`

**Closed**, and the zero was the tip of a deeper finding: the annotation
was not in `BUILTIN_ANNOTATIONS`, and a module declaration never had its
annotations applied at all. A whole module declared nested emitted
`IS_NESTED`-free TypeObjects — a topic type per endpoint the author did
not want. `Ast::effective_nested` now walks the module chain. Test:
`crates/idl-lower/tests/annotations.rs::default_nested_on_a_module_reaches_the_types_in_it`.

The other 23 annotations of the axis and all 28 TypeKinds have at least
one occurrence. That an occurrence is not an exercise is said above.

---

## E — deliberate, with a reason

Not a gap. Each line names its record; the page keeps them apart so they
stay distinguishable.

### E.1 The must-understand bit on a union discriminator is always set

`crates/types/src/from_ast.rs` sets
`UnionDiscriminatorFlag::IS_MUST_UNDERSTAND` unconditionally and gives
its reason on the spot: §7.2.2.4.4.4.6 ("shall always be true") and
`docs/omg-xtypes-issues.md` §B2, which records that no implementation
sets this bit. That is a deliberate departure from measured vendor
behaviour in favour of the specification's text.

### E.2 The placeholder in the SCC hash is stated openly

`from_ast.rs` carries `Scc::PLACEHOLDER_IS_UNSETTLED` and says what is
measured and what is not: the order, the numbering from one, the fourteen
MD5 octets and the XCDR2 stream are the specification's, the placeholder
of step 1 is not, and no implementation in the interop matrix computes an
SCC identifier to check it against. That is not a gap against the
specification but a disclosed uncertainty.

### E.3 `EnumTypeFlag` follows the vendors against Annex B

Annex B: `typedef TypeFlag EnumTypeFlag; // Unused. No flags apply`.
Table 18 nevertheless requires comparing the extensibility of two
enumerations. Measured: RTI Connext 7.7.0, OpenDDS and Cyclone write the
`TypeFlag` extensibility position into the word and `IS_DEFAULT` (bit 6)
on the first literal of an enumeration without `@default_literal`; Fast
DDS writes neither. RTI rehashes a received enumeration from its own
rendering and, without the bits, did not find the object under the
announced hash. The tree writes the bits as the three vendors do;
`docs/omg-xtypes-issues.md` §B8 holds the contradiction and the
measurement.

---

## What this survey does not cover

- **No vendor comparison inside the probes.** None of the 201 probes
  runs against Fast DDS, Cyclone, OpenDDS or RTI; the probe binary talks
  to none of them. For C.1/C.2 the comparison has since been run
  *beside* them and recorded —
  `crates/types/tests/typeobject_vendor_dheader.rs` holds Cyclone's and
  RTI's octets, `tools/typeobject-bytes/record.sh` re-derives them — but
  it is a test of its own and not a column of this survey.
- **The XML type representation (§7.3.2) and the XSD representation
  (§7.3.3) are unchecked.** Both have tables of their own (26, 27, 28)
  and would each have needed an extractor; `crates/xml` implements them
  and this survey does not touch it.
- **The XCDR encoding rules (§7.4.3.5) are unchecked.** The serialization
  virtual machine with its Tables 36 through 40 is the largest single
  table in the specification and the axis on which `tools/byte-gate` and
  `tools/wire-oracle` measure — it would have doubled this survey.
- **The dynamic language binding (§7.5.2) is unchecked.** Tables 48
  through 58 describe 11 interfaces with their operations; what is
  measured here is only that a `CompleteTypeObject` becomes a model (the
  `rule` column of the `to` probes).
- **Endianness in Table 60.** What is measured are the six pairs of
  representation and extensibility. Which of the twelve identifier values
  the encoder actually writes is decided in
  `crates/rtps/src/participant_message_data.rs`, and this survey does not
  touch it.
- **`corpus` is a string search.** A hit does not prove the file
  exercises anything. A zero proves the string does not occur in the
  tree.
- **The `assign`, `annotation` and `datarep` axes have a hand-assembled
  denominator.** Every single rule is backed; that they are all of them
  is not checked.

## The tool

```
tools/xtypes-1.3-gap-scan/roster.py           pull the denominator and check it
tools/xtypes-1.3-gap-scan/roster.py --check   check only, write nothing
tools/xtypes-1.3-gap-scan/scan.sh             measure and compare
tools/xtypes-1.3-gap-scan/scan.sh --record    rewrite expected.tsv
tools/xtypes-1.3-gap-scan/scan.sh --report    print the full measurement
tools/xtypes-1.3-gap-scan/scan.sh --explain   with the first diagnostic per probe
PROBE=/path/to/xtypes-gap-probe tools/xtypes-1.3-gap-scan/scan.sh
```

Exit codes of `scan.sh`:

| | |
|---|---|
| 0 | measured; the gap is exactly what `expected.tsv` records |
| 1 | a probe that used to get through no longer does — the gap grew |
| 2 | nothing was measured: no probe binary. Never a pass. |
| 3 | a probe now gets through that `expected.tsv` records as failing — the gap shrank and this page is behind the tree |
| 4 | `cases.tsv` and `expected.tsv` name different probes |

Exit codes of `roster.py`: 0 every check holds, 1 a completeness check
failed (and `roster.tsv` is then **not** written), 2 no PDF or no
`pdftotext` — nothing was extracted.

All five paths of `scan.sh` and all three of `roster.py` have been
exercised.

Runtime on an M1: 10 seconds for 199 probes over 10,722 committed
sources. The probe binary is a workspace of its own under
`tools/xtypes-1.3-gap-scan/probe/`; it is deliberately not in the root
member list, so that a measuring instrument does not touch the product's
`Cargo.lock`.

## Method note — what XTypes cost against IDL 4.2

The predecessor's estimate was "one day per spec for the extractor plus
the probes". It holds, but the distribution is a different one.

**What was cheaper here.** The probes. An octet stream from Annex B is
written on one line (`80F3000020 04` is a plain sequence of `long`), and
the driver is one program with seven ops rather than 356 separate files.
Of the 199 probes, two had to be reworked because they failed for the
wrong reason — the predecessor needed two full rounds of that.

**What was more expensive here.** The denominator. IDL 4.2 has Annex A;
XTypes has seven tables in seven shapes, and `roster.py` is 437 lines for
what was twenty lines of Python there. Three traps cost a round each: `pdftotext` puts a
form feed in front of every clause that lands at the top of a page, so
six of 24 clause numbers are not at the start of a line and the hole
check reports a hole the document does not have; the table of contents
repeats every table caption, so a search over the full text finds the
entry rather than the table; and Table 29 contains the string `7.3.4.3`
in one of its own cells, so a block that ends there stops before its last
row — the very row that carries finding C.4.

**Where the full text is needed.** Unlike IDL 4.2, for every category.
The grammar axes (`typekind`, `typeid`, `typeobject`, `typelookup`) fall
out of Annex B's IDL; the axes carrying 24 of the 33 findings (`assign`,
`annotation`, `datarep`) live entirely in running text and table cells.
Without the PDF in full text they would have been unfindable. The same
text refuted two claims that would otherwise stand here as gaps: a
`TI_STRING8_SMALL` with bound 0 is not an invalid bound but the unbounded
string (Table 29: "It is used for unbounded strings"); and an array of
`@appendable` elements *is* strongly assignable, because §7.2.4.2 makes
appendable types delimited under encoding version 2.

**What that means for CORBA 3.3.** The predecessor's recommended order
held up: the wire area is the one where an overlooked construct quietly
delivers wrong data instead of an error message — six of the nine B lines
here are exactly that. For CORBA the cost split is the other way round:
Part 1 Annex A carries the consolidated grammar, so the denominator is
cheap again, and the running text is ~1,150 pages over three parts, eight
times IDL 4.2. The cost driver there will be the B axis, and that needs
the full text clause by clause. One day per part is the realistic
estimate, not one day for CORBA.

**One rule this survey learned.** Where the specification contradicts
itself, the question is not whether the tree is wrong but which of the
two sentences it follows. Three findings here are of that kind (A.8,
B.8/B.9, C.4), and each of them would have been written down wrongly as
"a gap in the tree". `docs/omg-xtypes-issues.md` is the place for that
half.

## Decision records

The survey itself makes none. The implementation made four, and they are
written where they apply in the source rather than here:

1. **`@key` on a union discriminator is settled on an axis, not by the
   front end.** `crates/ast/src/facet/xtypes.rs`, `discriminator_is_key`,
   and `zerodds_idl_lower::KeyOnUnionDiscriminator`. Table 19 needs the
   bit in the TypeObject; three measured vendors refuse a
   keyed-discriminator IDL outright, so a KeyHash over it would be one
   nobody else computes. `Refused` is the default and reports the
   annotation with the clause that permits it; `Carried` records it and
   the KeyHash follows.
2. **`EnumTypeFlag` carries the extensibility bits and
   `IGNORE_LITERAL_NAMES` at bit 5.** `crates/types/src/from_ast.rs`,
   `enumeration`. The extensibility bits follow the measurement in E.3;
   §7.3.1.2.1.11 names a behaviour with no other place to live, and bit 5
   sits above the five `TypeFlag` positions.
3. **The `TypeObject` DHEADER lives in the encoder, not in the hash
   path.** `crates/types/src/type_object/mod.rs`, `encode_into`. Two
   paths counting it differently are worse than one that writes it
   nowhere.
4. **An `@annotation` declaration gets no `TypeNode`.**
   `crates/types/src/from_ast.rs`, `annotation_type_object`. A `@Name`
   should resolve to the declaration; the TypeObject is built from the
   declaration rather than from a type that does not exist.
