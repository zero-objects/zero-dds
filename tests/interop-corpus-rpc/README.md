# The RPC corpus

Cases for the language bindings of DDS-RPC 1.0, beside `tests/interop-corpus`.

The cases of `tests/interop-corpus` are data samples: each has a top-level
type, a `values.json` and an expected wire, and a dozen tools read them
(`tools/interop-results`, `tools/wire-capture`, the wire probes). An
`interface` has no sample, so its cases live here and only the generator
gates (`crates/lang-rules/tests/{cpp,java}_corpus_compiles.rs`) read them.

```text
<case>/
    type.idl     the IDL: interfaces, with inheritance, an attribute,
                 raises, out/inout, and (Enhanced) @Mutable
    mapping      basic | enhanced: the service mapping the case is
                 generated for (DDS-RPC 1.0 §7.5.1.1, §7.5.1.2)
```
