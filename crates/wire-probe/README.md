# zerodds-wire-probe

IDL-and-JSON measurement support for the ZeroDDS interoperability suite.
The probe walks the model and layout plan rather than compiling a generated
application, so its measurements are distinct from code-generator tests.
The execution engine is [zerodds-wire-codec](../wire-codec/README.md).

The JSON representation is type-directed. In particular, 64-bit integers and
`fixed<P,S>` decimals use strings, maps use arrays of key/value pairs, unions
carry `_d` and the active member, and an absent optional member is omitted
rather than set to null. See [the library reference](src/lib.rs) for the
complete representation table and exposed modules.

[Tests](tests) cover the reflective path and foreign-column fixtures.
Measurement orchestration lives under [tools/wireprobe](../../tools/wireprobe).
Neither a successful self-roundtrip nor an emitted vendor source file alone
proves cross-vendor interoperability. The
[original interop-suite design](../../docs/specs/zerodds-interop-suite-1.0.md)
describes the broader evidence model, not today's coverage counts.
