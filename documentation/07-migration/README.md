# 07 – Migration Guides

Already running on another DDS implementation? This station is the
porting playbook. The wire is RTPS-2.5-byte-identical across all
mainstream vendors, so application-level migrations are mostly a
matter of API shape and tooling.

## Available migration guides

| From | Guide |
|---|---|
| Eclipse Cyclone DDS (cyclonedds) | [from-cyclonedds.md](from-cyclonedds.md) |
| eProsima Fast DDS (FastDDS / Fast-RTPS) | [from-fastdds.md](from-fastdds.md) |
| RTI Connext DDS (Pro / Micro) | [from-rti-connext.md](from-rti-connext.md) |
| ADLINK / OpenDDS | [from-opendds.md](from-opendds.md) |
| Vortex OpenSplice (legacy) | covered under ADLINK Vortex / OpenDDS |

## What is the same

- **Wire format** — every mainstream DDS vendor speaks DDSI-RTPS
  2.5 + CDR / XCDR2 per OMG-XTypes 1.3. ZeroDDS interoperates
  byte-for-byte with all of them. You can run a hybrid fleet
  during the migration: half on the old vendor, half on ZeroDDS,
  and they discover and exchange samples normally.
- **OMG IDL 4.2** — IDL files port without changes. ZeroDDS'
  `zerodds-idlc` accepts the standard grammar; vendor-specific
  pragmas are silently ignored or mapped to the closest XTypes
  annotation.
- **QoS semantics** — all 21 standard QoS policies are
  implemented; per-policy match rules follow the OMG spec.
- **Discovery** — SPDP multicast `239.255.0.1:7400 + 250×D` is
  the same wire-level protocol; no special configuration to
  interoperate.

## What is different

| Aspect | Other vendors | ZeroDDS |
|---|---|---|
| API style | each vendor has its own (Cyclone-C, FastDDS-C++, RTI-Modern-C++, Java PSM …) | We ship the OMG-spec PSM where one exists (Java, Python, TypeScript, C++17) plus an idiomatic Rust API |
| QoS profile XML | vendor-specific schemas | We accept the OMG `zerodds-xml-1.0` schema; conversion guides linked per vendor below |
| Security plugins | typically vendor-built-in or external commercial product | Built-in OMG DDS-Security 1.2 — no extra licence |
| Build system | C/C++ make + IDL-tooling | Cargo for Rust + per-language packaging (Maven / npm / pip / NuGet) |
| RT integration | varies | Linux-RT scheduler + isolcpu via the dedicated `zerodds-rt-linux` crate |

## Hybrid-deployment recipe

You don't have to flip the switch all-at-once. Typical migration:

```
Day 0: 100% Cyclone DDS
Day 1: pilot one Subscriber on ZeroDDS, traffic from existing
       Cyclone-Publishers — verify byte-compat
Day 2-N: gradually swap nodes
Day N+1: 100% ZeroDDS, retire the old install
```

Cross-vendor smoke tests live in
[`../../docs/interop/`](../../docs/interop/) and run nightly
against Cyclone DDS + FastDDS + (when licensed) RTI Connext.

## What you save

The most common migration drivers in the current ZeroDDS
deployments:

| Driver | Why ZeroDDS |
|---|---|
| Memory safety / supply-chain | Pure Rust, no C/C++ memory bugs in the hot path |
| RT determinism | Tight allocation control + RCU history cache + `zerodds-rt-linux` integration |
| ROS-2 with smaller footprint | `rmw-zerodds-shim` replaces `rmw_cyclonedds_cpp` / `rmw_fastrtps_cpp` |
| Vendor lock-in escape | Apache-2.0; no per-seat licence; bring-your-own-CA security |
| Single binary on Windows / macOS | First-class platform installers; many other vendors are Linux-first |

## Common cross-vendor caveats

- **Multicast filtering**. Cloud / VPC environments often disable
  IGMP-snooping. Workaround is unicast static peer-list — same
  code on every vendor.
- **Permissions XML signing**. The OMG-DDS-Security spec mandates
  S/MIME-signed permissions XML; some vendors ship convenience
  CLIs (`rti_secure_credential_tool`) for it. ZeroDDS uses
  `openssl smime`; see
  [`../03-configuration/security.md`](../03-configuration/security.md).
- **Vendor-specific QoS extensions**. RTI Connext for example
  has `RtPsResource…` policies not in OMG-DDS-1.4. ZeroDDS
  ignores unknown pragmas. If you depend on them, the migration
  guide for that vendor flags equivalents or workarounds.

## Reading further

- [Wire interop test harness](../../docs/interop/) — how we
  verify byte-compat per vendor.
- [Spec coverage matrix](../../docs/spec-coverage/) — which
  DDS-spec sections ZeroDDS ships fully.
- [Per-language integration](../05-integration/README.md) —
  pick your binding after migration.
