# zerodds-codec

A DDS topic type defined in Rust instead of IDL.

```rust,ignore
use zerodds_codec::{DdsType, WireRepr};

#[derive(DdsType)]
#[dds(name = "app::Track", extensibility = "appendable")]
struct Track {
    #[dds(key)]
    id: core::num::NonZeroU32, // unsigned long
    at: Dtg,                   // string, through the Dtg's WireRepr
    #[dds(bound = 64)]
    label: String,             // string<64>
    note: Option<String>,      // @optional string
}

println!("{}", Track::idl()?); // the IDL a C or C++ peer is generated from
```

`derive(DdsType)` writes the type's description (`DdsTypeInfo`), its
codec (`DdsSerialize`, `DdsDeserialize`), its `DdsMember` impl for use as a
member of another type and the `zerodds_dcps::DdsType` impl a `DataWriter`
and a `DataReader` take. `derive(WireRepr)` makes a checked newtype stand
for an IDL `string` or primitive.

The description becomes IDL, the IDL goes through the same front end, the
same type-object builder and the same layout planner as a `.idl` file given
to `zerodds-build`, and the plans drive the serializer of `zerodds-wire`.
A derived type and the type generated from the equivalent IDL have the same
`TypeObject`, the same `TypeIdentifier` and the same bytes under XCDR1 and
XCDR2 in both byte orders.

A hand-written codec is checked at compile time: the serializer is typed by
the member list of the description, so a member left out, one too many or
one of another type does not compile. See the `codec` module of
`zerodds-wire`.

An enum whose variants carry data is an IDL `union` (`discriminator`,
`case` labels written or counted, `default`, `no_member`, branch ids); a
generic type is one IDL type per instantiation, named from its arguments
(`Pair<u32>` is `Pair_unsigned_long`; `#[dds(name = "app::Pair_{T}")]`
sets the template); a field may borrow from the received sample (`&'a str`,
`&'a [u8]`, `Cow<'a, str>`, `Cow<'a, [u8]>`), which `decode` lends out of
the bytes. A type with a borrowed field has no `zerodds_dcps::DdsType` impl:
the DataReader copies into an owned `T` until the loaned take exists.

License: Apache-2.0
