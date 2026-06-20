# 04 – IDL Reference

ZeroDDS uses [OMG IDL 4.2][idl] as the wire-type definition
language. Types you write in `.idl` are compiled by `zerodds-idlc`
into Rust / C++ / C# / Java / TypeScript / Python stubs.

## Sub-stations

- [`zerodds-idlc` handbook](idlc-handbook.md) — the comprehensive
  end-user reference: install, CLI, the full annotation set (`@key`,
  `@id`, `@final`/`@appendable`/`@mutable`, `@hashid`, `@verbatim`, …),
  build integration, cookbook, troubleshooting.
- [CDR wire format](cdr-wire-format.md) — XCDR1 / XCDR2 byte form
  for every IDL construct, alignment rules, debugging tips.

## TL;DR

`Robot.idl`:

```idl
module Robot {
    @final
    struct Pose {
        @key string<32> robot_id;
        double x;
        double y;
        double z;
        double yaw;
    };

    @appendable
    struct Telemetry {
        @key string<32> robot_id;
        Pose pose;
        sequence<double, 32> joint_angles;
        unsigned long long t_nanos;
    };
};
```

Compile:

```bash
zerodds-idlc Robot.idl --rust  -o gen/rust
zerodds-idlc Robot.idl --cpp   -o gen/cpp
zerodds-idlc Robot.idl --java  -o gen/java
zerodds-idlc Robot.idl --csharp -o gen/cs
zerodds-idlc Robot.idl --ts    -o gen/ts        # per DDS-TS 1.0 spec
zerodds-idlc Robot.idl --python -o gen/py
```

Each backend produces:

- A type definition (Rust struct, C++ class, Java POJO, C# class,
  TS interface, Python dataclass).
- A CDR encoder + decoder.
- A KeyHash computation per `@key`.
- A registration helper for DataWriter / DataReader.

## Conformance

ZeroDDS supports the full OMG IDL 4.2 grammar plus:

- DDS-XTypes 1.3 annotations (`@final`, `@appendable`, `@mutable`,
  `@key`, `@id`, `@hashid`, `@verbatim`).
- Bitset and bitmask types (Spec §7.4.13).
- `@try_construct` (Spec §7.2.4.4).
- Inheritance (`@extensibility(MUTABLE)`).

## Next

- [`zerodds-idlc` handbook](idlc-handbook.md) for the CLI, annotations
  and build integration in depth.
- [Integration — Java](../05-integration/java.md) /
  [TypeScript](../05-integration/typescript-wasm.md) to wire the
  generated stubs into your application.

[idl]: https://www.omg.org/spec/IDL/4.2/
