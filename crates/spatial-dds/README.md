# zerodds-spatial-dds

SpatialDDS profile types generated at build time from the vendored
[spatial_core.idl](idl/spatial_core.idl). The crate exposes the generated
`spatial` and `builtin` modules with inherent encode/decode methods using
`zerodds-wire`. The [build script](build.rs) selects framing per aggregate's
extensibility annotation.

The input derives from SpatialDDS 1.6, but is **not byte-for-byte upstream
IDL**. Its header records identifier corrections, including renamed members
that affect Complete TypeObjects and potentially derived mutable member IDs.
Compatibility with an unmodified upstream-generated peer must therefore not
be assumed from the profile name alone.

See [the library](src/lib.rs) for module organization and [tests](tests)
for local codec coverage. This crate supplies a type layer, not a running
transport or an independent vendor-interop certification. Semantic ROS 2
conversion is provided by [zerodds-spatial-ros2](../spatial-ros2/README.md).
