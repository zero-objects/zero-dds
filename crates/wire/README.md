# `zerodds-wire`

The runtime a generated encoder calls.

`Writer` and `Reader` put bytes on the wire and take them off, in XCDR1
and XCDR2, in either byte order. They hold the alignment rules and the
framing forms — DHEADER, EMHEADER, PL_CDR1 parameter headers, the
PID_LIST_END sentinel — and nothing else.

This crate makes **no decisions about a type**. Which members are
written, in what order, with which header and which length code, is the
layout plan's answer and reaches this crate as arguments. A runtime that
decided any of it would be a second place where the wire format lives,
and the two would drift.

See `docs/specs/zerodds-ast-1.0.md` §7 for the layout plan this serves.

## A Rust type for an IDL primitive or `string`

`WireRepr` lets a Rust type stand for an IDL primitive or `string` in
the code `zerodds-build` generates (`Config::rust_type`): a checked
date-time newtype for a `string`, `NonZeroU32` for an `unsigned long`.
The octets, the `TypeObject`, the `TypeIdentifier` and the key hash stay
the IDL's.

```rust
use zerodds_wire::{IdlOut, WireError, WireRepr, Written};

#[derive(Debug, Clone, PartialEq)]
pub struct CallSign(String);

impl WireRepr for CallSign {
    type Idl = str; // an IDL `string`
    fn to_wire(&self, out: IdlOut<'_, str>) -> Result<Written, WireError> {
        out.str(&self.0)
    }
    fn from_wire(wire: &str) -> Result<Self, WireError> {
        if (2..=8).contains(&wire.len()) {
            Ok(Self(wire.to_owned()))
        } else {
            Err(WireError::invalid_value("a call sign of 2 to 8 octets"))
        }
    }
}
```

- `Idl` names the IDL type: `str` for a `string`, `u32` for an
  `unsigned long`, and so on. `to_wire` gets an `IdlOut` for exactly that
  type and returns the `Written` only an `IdlOut` makes, so an impl
  writes one value of the member's IDL type.
- `IdlOut::str` writes a borrowed `&str`; `IdlOut::display` formats a
  `Display` value straight into the sample's buffer and patches the
  length in afterwards. The bound of a `string<N>` and the NUL check run
  on the written text.
- `from_wire` gets `&str` lent out of the received sample
  (`Reader::read_str`), with no allocation. A refusal
  (`WireError::invalid_value`) rejects the sample with its reason.
- Impls in this crate: `core::num::NonZero*` over the integers, `String`,
  `Cow<'_, str>` and each primitive's own Rust type. A type from another
  crate goes through an adapter, `WireReprWith`; `DisplayFromStr` is the
  one for any `Display + FromStr` type on a `string`.

## A type defined in Rust: the codec API

The `codec` module has the traits a type defined in Rust implements —
`DdsTypeInfo` (its IDL shape: the members' Rust types as a type list, the
declaration as a constant), `DdsSerialize` and `DdsDeserialize<'de>` —
and the serializer and deserializer that execute the type's layout plan,
one per encapsulation. `derive(DdsType)` of `zerodds-codec` writes all
three. A hand-written impl is checked at compile time: `member` takes a
value of the next member's type and `end` exists only when no member is
left. A union is typed the same way: `branch::<B1>(&discriminator,
&value)` takes the branch as a type-level index and a value of exactly its
type. A member read through `DdsMemberRead<'de>` may borrow from the
received bytes (`&'de str`, `&'de [u8]`, `Cow<'de, str>`, `Cow<'de, [u8]>`).
`core::time::Duration` is the DDS `Duration_t` and, with the `std`
feature, `std::time::SystemTime` is `Time_t`.
