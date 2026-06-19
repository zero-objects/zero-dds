# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the `zerodds-corba-ior` crate.

### Spec references

- **OMG CORBA 3.3 Part 2**: §13.6 (Object References),
  §13.6.2 (IOR struct), §13.6.7.1 (profile tags),
  §13.6.7.3 (TaggedComponents), §13.6.10 (stringified IOR +
  corbaloc/corbaname URL schemes), §15.7.2 (IIOP ProfileBody content
  of the `TAG_INTERNET_IOP` profile), §10 (CSIv2 CompoundSecMechList).

### Public API

- `ior::Ior` — IOR struct (`type_id` + `Vec<TaggedProfile>`).
- `tagged_profile::TaggedProfile` — profile variants
  (`MultipleComponents`, `InternetIop` with `IiopProfileBody` from
  `corba-iiop`, plus an opaque variant).
- `profile_tags::ProfileId` — `InternetIop`, `MultipleComponents`,
  `ScCorbalocImr` and vendor tags.
- `components::{TaggedComponent, StructuredComponent,
  AlternateIiopAddress, CodeSetComponent, CodeSetComponentInfo,
  OrbType, Ssl, StreamFormatVersion, TlsSecTrans}` — all 32
  standard component tags including structured decoders for:
  ORB_TYPE / CODE_SETS / ALTERNATE_IIOP_ADDRESS / SSL_SEC_TRANS /
  TLS_SEC_TRANS / RMI_CUSTOM_MAX_STREAM_FORMAT / JAVA_CODEBASE /
  CSI_SEC_MECH_LIST.
- `component_tags::ComponentId` — `unsigned long` tags.
- `stringified::{STRINGIFIED_IOR_PREFIX, from_stringified,
  to_stringified}` — `IOR:` prefix + hex encoding of a CDR
  encapsulation (spec §13.6.10).
- `url::{CorbalocAddress, CorbanameAddress, parse_corbaloc,
  parse_corbaname}` — URL parsers for the `corbaloc:` and `corbaname:`
  schemes.
- `error::{IorError, IorResult}` — error surface.

### Implementation

`#![cfg_attr(not(feature = "std"), no_std)]` with `extern crate alloc`;
`#![forbid(unsafe_code)]`.

CDR encapsulation per IOR roundtrip; endianness marker in the first
byte per the CDR-1 spec. `to_stringified`/`from_stringified` round-trip
strictly through hex encoding.

`StructuredComponent::CsiSecMechList` is the typed form of the
`TAG_CSI_SEC_MECH_LIST` tag (CSIv2 §10), with cross-crate wire-up via
`zerodds-corba-csiv2::CompoundSecMechList`. This externally resolves
F-CORBA-CSIV2-NOT-WIRED.

URL parsers accept multi-address syntax (`addr,addr,...`) per
§13.6.10.

### Architecture

- **Layer:** 8 (CORBA stack, Tier-C).
- **Dependencies (in):** `zerodds-cdr`, `zerodds-corba-csiv2`,
  `zerodds-corba-iiop`.
- **Dependents (out):** `zerodds-corba-cosnaming` (NameBinding object
  refs), `zerodds-corba-dds-bridge` (object_key mapping).
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Wire format fixed by OMG.
- `CsiSecMechList` variant new in rc1 (cross-crate resolution of
  F-CORBA-CSIV2-NOT-WIRED).
