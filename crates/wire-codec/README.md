# zerodds-wire-codec

Reflective execution of a ZeroDDS AST layout plan against a value, in both
directions. The crate provides encoding, decoding, canonicalization, defaults
and value-contract checking. It does not parse IDL or operate DDS endpoints.

`Source` reads a value for encoding; `Sink` constructs a value during decoding.
This keeps execution independent of any one value representation. The
default `serde-json` feature supplies the JSON implementation and can be
disabled by consumers using another representation.

Framing and dialect choices come from `zerodds-ast-layout`; byte operations
come from `zerodds-wire`. See the [library entry point](src/lib.rs),
[Source](src/source.rs), [Sink](src/sink.rs) and [tests](tests).
For an IDL/JSON measurement front end, see
[zerodds-wire-probe](../wire-probe/README.md).
