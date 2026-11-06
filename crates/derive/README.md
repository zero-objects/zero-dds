# zerodds-derive

`derive(DdsType)` and `derive(WireRepr)` for
[`zerodds-codec`](https://docs.rs/zerodds-codec), which re-exports both.
Depend on `zerodds-codec`, not on this crate.

- `derive(DdsType)` on a struct with named fields: a DDS topic type defined
  in Rust. Attributes mirror the IDL annotations: on the struct `name`,
  `extensibility`, `autoid`, `nested`, `codec = "manual"`; on a field `key`,
  `id`, `hashid`, `bound`, `name`, `optional`, `with`.
- `derive(DdsType)` on a field-less enum: an IDL `enum`, with `name` and
  `bit_bound`, and `name` and `value` on a variant; it also serves as a
  union's discriminator type.
- `derive(DdsType)` on an enum whose variants carry data: an IDL `union`.
  On the enum `discriminator = "Type"` (default `i32`), beside the struct's
  attributes; on a variant `case = label` or `case = [label, …]`, `default`,
  `no_member`, `id`, `hashid`, `name`, `bound`. A variant `V(T)` holds the
  branch; `V(D, T)` also holds the discriminator, which a case with several
  labels and the `default:` case need.
- Generic types (one IDL type per instantiation, the name from a template
  such as `"app::Pair_{T}"`) and borrowed fields (`&'a str`, `&'a [u8]`,
  `Cow<'a, …>`, lent out of the received sample by the codec API).
- `derive(WireRepr)` with `#[wire(idl = "string")]`: a checked type read
  through `TryFrom<&str>` and written through `Display`; with an IDL
  primitive (`#[wire(idl = "uint32")]`), read through `TryFrom<u32>` and
  written through `Into<u32>`.

License: Apache-2.0
