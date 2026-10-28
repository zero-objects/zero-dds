# zerodds-corba-codegen

The CORBA 3.3 Annex A.1 special types, and the stub and skeleton
mappings that go with them, over the common AST. `no_std + alloc`,
`forbid(unsafe_code)`.

Annex A.1 lists thirteen IDL constructs whose language mapping is not the
mechanical one. Everything else — a `long`, a `struct`, a
`sequence<long>` — takes the ordinary rule its PSM defines. This crate
answers, for a type in the model, which of the thirteen it is and what
each language calls it.

## The table

| IDL | C++ | C# | Java | Rust |
|---|---|---|---|---|
| `Object` | `CORBA::Object_var` | `omg.org.CORBA.Object` | `org.omg.CORBA.Object` | `zerodds_corba_rt::ObjectReference` |
| `ValueBase` | `CORBA::ValueBase` | `omg.org.CORBA.ValueBase` | `java.io.Serializable` | `zerodds_corba_rt::ValueBase` |
| `AbstractBase` | `CORBA::AbstractBase_var` | `omg.org.CORBA.AbstractBase` | `org.omg.CORBA.portable.IDLEntity` | `zerodds_corba_rt::ValueBase` |
| `native` | `void*` | `object` | `java.lang.Object` | `core::ffi::c_void` |
| `TypeCode` | `CORBA::TypeCode_var` | `omg.org.CORBA.TypeCode` | `org.omg.CORBA.TypeCode` | `zerodds_corba_rt::TypeCode` |
| `any` | `CORBA::Any` | `omg.org.CORBA.Any` | `org.omg.CORBA.Any` | `zerodds_corba_rt::CorbaAny` |
| `sequence<any>` | `CORBA::AnySeq` | `omg.org.CORBA.AnySeq` | `org.omg.CORBA.Any[]` | `Vec<zerodds_corba_rt::CorbaAny>` |
| `string` | `CORBA::String_var` | `string` | `java.lang.String` | `String` |
| `wstring` | `CORBA::WString_var` | `string` | `java.lang.String` | `zerodds_corba_rt::WString` |
| `TimeBase::TimeT` | `TimeBase::TimeT` | `ulong` | `long` | `u64` |
| `fixed<digits,scale>` | `CORBA::Fixed` | `decimal` | `java.math.BigDecimal` | `Fixed` |
| `unsigned long long` | `CORBA::ULongLong` | `ulong` | `long` | `u64` |
| `long double` | `CORBA::LongDouble` | `decimal` | `double` | `zerodds_float128::Float128` |

`mapping_table_markdown()` produces this table, so the crate and this
file cannot drift apart.

## What it is over

`zerodds-ast` for the model, `zerodds-lang-cst` and `zerodds-lang-render`
for the output. `special_type_of` takes an `Id<TypeNode>` out of
`zerodds-idl-lower`; the renderers build CST nodes, not strings.

## What the ORBs said

`tests/orb-corpus/rows/` writes each row in every argument position;
`record.sh` runs `omniidl`, `tao_idl` and JacORB's `idl` over it on the
measurement host and records what they generated. Four answers here come
from that recording and not from a reading of the specification:

- **A `typedef` is written by its own name.** `typedef fixed<10,2> Money`
  is `probe::Money` and not `CORBA::Fixed`; `native Handle` is
  `probe::Handle` and no ORB declares it.
- **An `abstract interface` is not the `AbstractBase` row.** All three
  write the interface's own name.
- **A Java `wstring` holder is `StringHolder`.** Both string kinds map to
  `java.lang.String`, so both take that holder.
- **A Java `sequence<any>` is `org.omg.CORBA.Any[]`.** The earlier table
  said `AnySeqHelper`, which is a marshalling class and never a member
  type.

Each ORB refused one row, and the refusal is recorded against that row
rather than worked around: omniidl's C++ backend stops at `native`,
`tao_idl` at `fixed`, and JacORB at `CORBA::TypeCode`.

## The argument-passing forms

IDL to C++ §1.22 gives `in`, `inout`, `out` and the return position four
different spellings of one type, and the `out` and return rows differ
between a fixed-length and a variable-length type — §1.9. `shape_of`
computes that property from the model's members rather than guessing it
from a name, because a `struct` holding one `string` is variable-length
and looks like any other struct. IDL to Java §1.4 routes `out` and
`inout` through holder classes instead.

## C\#

The C# column has an OMG mapping (`formal/2003-09-04`) and no compiler on
the measurement host, so it is cited and not measured.
`zerodds-lang-cst` has no C# tree either, so `render_stub_op` and
`render_skeleton_dispatch` refuse that column by name rather than
emitting C++ with C# names in it.

## Licence

Apache-2.0.
