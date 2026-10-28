# zerodds-xml-compose

DDS-XML type definitions into the ZeroDDS common AST — the XML
counterpart of `zerodds-idl-compose`.

One call takes a DDS-XML document and produces the same
`zerodds_ast::Ast` that the IDL front end produces from an `.idl` file,
so everything downstream of the model — `zerodds_types::from_ast`, the
layout planner, `zerodds-wire-codec`, every code generator — is unchanged
and does not know which language the declaration was written in.

```rust
let composed = zerodds_xml_compose::compose("types/xml/primitives.xml")?;
assert!(!composed.failed());
```

## Why not `zerodds-xml`

`zerodds-xml` reads DDS-XML already and stops one layer short: it builds
a descriptive `TypeLibrary` and bridges that to a `MinimalTypeObject`.
That layer is a published API with its own users, and it drops what a
model needs — a member carries no `tryConstruct`, no `hashid` and no
`external`, a struct carries no `autoid`, an enum carries neither
extensibility nor `defaultLiteral`, a union carries no extensibility, and
its primitive table has no `int8` or `uint8`. Widening those structs
would change signatures every current caller compiles against.

This crate therefore takes the position against `zerodds-xml` that
`zerodds-idl-compose` takes against `zerodds-idl-parse`: it reuses the
front end's well-formedness layer — `parse_xml_tree` and its element
tree, with the DDS-XML 1.0 §7.1 depth cap, element budget and namespace
handling — and lowers that tree into the model itself.

## What it carries

`<module>`, `<struct>` (with `extensibility`, `baseType`, `autoid`),
`<union>` (with `<discriminator>` and `<case>`/`<caseDiscriminator>`,
including `default` and enumerator-name labels), `<enum>` (with
`bitBound`, `extensibility`, `defaultLiteral`), `<bitmask>` (both the
`<flag value=…>` and `<bit_value position=…>` spellings), `<typedef>`,
`<forward_dcl>`; every XTypes and IDL primitive spelling; bounded and
unbounded strings and wstrings; sequences, maps and multi-dimensional
arrays; `<const>` (at the top, in a module and in a structure) and
`<const_exp>` for bounds, array dimensions and enumerator values;
`<include>`; `<bitset>`; `<verbatim>`; `<annotation>` and `<annotate>`; and the member attributes `key`,
`optional`, `external`, `mustUnderstand`, `id`, `hashid` and `tryConstruct`.

Member ids are assigned by DDS-XTypes 1.3 §7.3.1.2.1.1 with
`zerodds_idl_lower::hashed_member_id`, so a type declared once in `.idl`
and once in `.xml` reaches the same `TypeIdentifier` by construction.
`tests/idl_parity.rs` asserts exactly that, pair by pair.

## What it refuses

* `<discriminator key="true">` — the model has no slot for it and
  `UnionDiscriminatorFlag` has no bit for it, so accepting it would give
  a keyed and an unkeyed union the same `TypeIdentifier`. The IDL front
  end refuses the same construct for the same reason.
* `<annotation baseType="…">` — the model's annotation declaration has
  no base.
* A built-in annotation written as `<annotate>` (`key`, `optional`, …);
  this representation writes those as attributes (§7.3.2.5).
* An `<include>` of a file that is not an XML document, and one that
  cannot be read.
* Every element it does not know, in a diagnostic naming it.

## Bounds

`-1` is unbounded. A bound of `0` is refused (IDL takes a positive
integer); `ComposeOptions::zero_bound_is_unbounded` reads it as unbounded,
which is how the OMG `dds-xtypes` suite writes it.

## Spec sources

* OMG DDS-XML 1.0 (formal/18-10-01) §7.1, §7.3.3.
* OMG DDS-XTypes 1.3 Annex A (XML Type Representation), §7.3.1.2.1.
* OMG IDL 4.2 §7.4 where the two representations share a rule.
