# zerodds-proto

Protocol Buffers **type reuse** for DDS. A `FileDescriptorSet` (from
`protoc -o out.pb --include_imports x.proto`) is mapped to an XTypes
`DynamicType`, so a `.proto` data model becomes a DDS topic type and rides the
**DDS-native XCDR wire** through ZeroDDS' reflective `DynamicData` codec — not a
protobuf payload. proto **field numbers map directly to XTypes member IDs**.

This is the type-system counterpart to
[`zerodds-grpc-bridge`](../grpc-bridge/README.md) (which bridges the gRPC
*protocol*): `zerodds-proto` reuses the *types*.

```text
foo.proto --protoc--> FileDescriptorSet --zerodds-proto--> DynamicType --XCDR--> DDS wire
```

Pure-Rust, `no_std + alloc`. Part of
[**ZeroDDS**](https://github.com/zero-objects/zero-dds). Safety classification:
**STANDARD**. Apache-2.0.
