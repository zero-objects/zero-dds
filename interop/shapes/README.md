# The shape IDLs

`ShapeType` is the de-facto interop type of the DDS world, and there are
**three declarations under that one name** in this directory. They are
not versions of each other and none supersedes the others: three
different sets of peers speak them, and a sample of one is a mis-framed
sample to a reader of another.

Two more files here are not a fourth `ShapeType`.
`omg-dds-xtypes.idl` declares twenty-five types and calls none of them
`ShapeType`; `shapesdemo-extended.idl` declares one type under a
different name, `ShapeExtendedType`, which SEDP matches only against
other `ShapeExtendedType` endpoints.

All five are here so that a reader who finds one of them finds the
others in the same directory.

| file | type(s) | extensibility | members | key | XCDR2 framing | encapsulation id | who speaks it |
|---|---|---|---|---|---|---|---|
| `omg-dds-rtps.idl` | `ShapeType` | `@appendable` | 5 (`additional_payload_size` included) | `@key string<128> color` | DELIMITED_CDR — leading `uint32` DHEADER | `D_CDR2_LE` = `0x00 0x09` | the OMG **`dds-rtps`** interoperability suite; `tools/omg-shape-main` |
| `omg-dds-security.idl` | `ShapeType` | none written — APPENDABLE by XTypes 1.3 §7.3.1.2.1 | 4 | `string<COLOR_MAX_LENGTH> color; //@key` | DELIMITED_CDR — leading `uint32` DHEADER | `D_CDR2_LE` = `0x00 0x09` | the OMG **`dds-security`** interoperability suite; `tools/omg-security-main` |
| `shapesdemo.idl` | `ShapeType` | `@final` | 4 | `@key string color` | PLAIN_CDR2 — no DHEADER | `CDR2_LE` = `0x00 0x07` | the interactive ShapesDemo GUIs (RTI `-dataType Shape`, CycloneDDS, Fast DDS); `zerodds-examples/cross-vendor-shapes/` |
| `omg-dds-xtypes.idl` | 25 types, none named `ShapeType` | all three, five types each | 4 or 5, in four arrangements | `@key` on `color` throughout | all three of Tab. 59 | `CDR2_LE`, `D_CDR2_LE` and `PL_CDR2_LE` = `0x00 0x0b` | the OMG **`dds-xtypes`** interoperability suite; `tools/omg-xtypes-main` |
| `shapesdemo-extended.idl` | `ShapeExtendedType` + `enum ShapeFillKind` | `@final` | 6 (`fillKind`, `angle`) | `@key string color` | PLAIN_CDR2 — no DHEADER | `CDR2_LE` = `0x00 0x07` | RTI Connext 7.x ShapesDemo in its default mode (no `-dataType Shape`); `crates/dcps/src/interop.rs` |

XTypes 1.3 Tab. 59 is what makes the framings differ; XTypes 1.3
§7.4.3.4.2 is the DHEADER itself.

## The `dds-security` form, and the two things to know about it

The OMG runs three interoperability suites, not one: `dds-rtps`,
`dds-security` and `dds-xtypes`. The first two each carry their own
`ShapeType` and the two are not the same declaration.

`omg-dds-security.idl` is `srcCxx/ShapeType.idl` of the `dds-security`
suite, verbatim. It differs from the `dds-rtps` form in four ways: the
string bound is the named constant `COLOR_MAX_LENGTH` = 64 rather than
the literal 128; there is no fifth `additional_payload_size` member
(this suite has no large-data run); there is **no extensibility
annotation at all**; and the key is written `//@key` in a trailing
comment.

**The missing annotation.** XTypes 1.3 §7.3.1.2.1 makes an absent
extensibility APPENDABLE, and that is what
`tools/omg-security-main/build.rs` asks for. What each vendor's own IDL
compiler applies to the same file is a separate question that the suite
does not answer, and it is unmeasured here — a vendor whose default is
`@final` frames the same four members without a DHEADER.

**The `//@key` comment.** The form is normative — XTypes 1.3 Table 24:
an annotation may be applied "alternately and equivalently … by
suffixing it with an annotation type name using slash-slash-at".
Measured on the tree this file was added to, our front end does not read
it: the line passes `parse` and `lower` without a diagnostic and `color`
comes out with `facets.is_key() == false`, so the generated type has no
key and every sample lands in one instance. Front-end work to read the
form is under way. The file stays as the suite wrote it either way — an
IDL that differs from the suite's no longer measures the suite. The
consequence is written out in `tools/omg-security-main/README.md`.
## `omg-dds-xtypes.idl` is a different kind of thing

`omg-dds-rtps.idl`, `omg-dds-security.idl` and `shapesdemo.idl` each
declare one type and vary the QoS around it. `omg-dds-xtypes.idl` varies the **type** and leaves
the QoS alone: 25 declarations, from five member layouts (a base; a
member appended; a member inserted in the middle; the members
reordered; the base inherited and appended to) crossed with five
extensibility declarations (none, `@final`, `@appendable`, `@mutable`,
`@mutable` with `@id`). Its suite runs one of them against another and
asks whether the two are assignable, which is XTypes 1.3 §7.2.4.4.4.4
and not a framing question at all.

Two things about it that a reader will otherwise get wrong:

- **`STR_LEN_MAX` is 32, not 128.** `omg-dds-rtps.idl` bounds `color` at
  `string<128>`; this one at `string<STR_LEN_MAX>` with 32. The two
  suites are unrelated and the numbers do not have to agree — but the
  bound is on the wire type (XTypes 1.3 §7.3.1.2.1.4 puts it in the
  `StringSTypeDefn`), so it reaches assignability, and copying one
  number into the other file would silently answer a different question.
- **`…Default` is not a sixth extensibility**, it is the absence of an
  annotation, and what that means is the consumer's choice.
  XTypes 1.3 §7.3.1.2.1.9 makes APPENDABLE the default, and the suite's
  own compatibility matrix treats Default and Extensible as the same
  throughout. `tools/omg-xtypes-main/build.rs` therefore states
  `default_extensibility(Appendable)` rather than leaving it to whatever
  the front end's default happens to be.

## Why mixing them is silent on one side and loud on the other

An `@appendable` reader is handed a `@final` body and reads
`color.length` as the DHEADER. CycloneDDS reports
`deserialization Square/ShapeType failed` — that direction is loud, and
it is the measurement recorded in `crates/dcps/src/interop.rs`.

The other direction is the one to watch: a `@final` reader handed an
`@appendable` body reads the DHEADER as `color.length`. Whether that is
loud depends on the value.

## What each file backs

`crates/dcps/src/interop.rs` holds a hand-written Rust `ShapeType`. It
is the OMG five-member `@appendable` form, and it is what all six live
interop tests publish and subscribe
(`fastdds_live_pub`, `fastdds_live_sub`, `liveliness_qos`,
`lifespan_qos`, `time_based_filter_qos`, `shapes_api_e2e`).

That hand-written type used to be a second source of truth beside the
IDL front end: `crates/dcps/tests/shapes_type_wire.rs` checks it against
byte sequences that were also derived by hand, so a defect in our own
generator could not fail it. `crates/dcps/tests/shapes_type_idl_agreement.rs`
closes that: it encodes the same values through the hand-written type
and through `omg-dds-rtps.idl` — parsed, lowered and planned by
`zerodds-idl-compose` / `zerodds-ast-layout`, then serialised by the
reflective `zerodds-wire-codec` walk, with no code generation in the
path — and compares the octets in all four modes the interop matrix
carries: `xcdr1-le`, `xcdr1-be`, `xcdr2-le`, `xcdr2-be`.

`zerodds-examples/cross-vendor-shapes/ShapeType.idl` is the `@final`
form and lives in the public example mirror, which is its own
repository. `shapesdemo.idl` here is the in-tree copy the agreement test
reads. Change one and change the other.

`omg-dds-security.idl` backs `tools/omg-security-main` and nothing else.
It has no hand-written counterpart anywhere in the tree and is not going
to get one: that client generates its type from this file from its first
commit, for the reason `tools/omg-shape-main` records. What checks it is
`tools/omg-security-main/src/shape_type.rs::tests` — that the generated
type announces a `TypeObject`, that its two byte orders are two
encodings, and that `HAS_KEY` and `key_hash()` are one decision with
`color` behind it.

`omg-dds-xtypes.idl` backs `tools/omg-xtypes-main`, and nothing else
reads it. That crate generates all 25 types from it at build time,
dispatches over them at run time, and holds them against the suite's own
`compatibility_matrix.xlsx` in
`tools/omg-xtypes-main/tests/compatibility_matrix.rs`. It is also the
first IDL in this tree with `@mutable` structs in it, which is how the
missing `ParameterRole` import in `zerodds-build`'s `RUNTIME_IMPORTS`
was found — until that was fixed, no `@mutable` struct compiled through
the generator at all.

`shapesdemo-extended.idl` backs the hand-written `ShapeExtendedType` in
`crates/dcps/src/interop.rs`, which the two shapes-demo examples reach
through `ZERODDS_SHAPE_EXTENDED=1`. That type had no IDL anywhere in the
tree until this file was written and nothing held its octets against
one: its member order, its `@final` framing and the four-octet width of
the `ShapeFillKind` discriminant were asserted only against byte
sequences derived by the same hand that wrote the codec — the same
hand-against-hand arrangement `shapes_type_wire.rs` was in.
`crates/dcps/tests/shapes_type_idl_agreement.rs` now runs the
`ShapeExtendedType` pair through the same measurement as the
`ShapeType` pair, over four samples and the four modes. Measured on the
tree this file was added to, the two paths agree octet for octet in all
sixteen comparisons.

## What this directory does not do

It runs nothing. It is five files and the reason there are five. The
harnesses that run are listed in
`docs/interop-harnesses.md`, which also records what each of them proves
and what it does not.
