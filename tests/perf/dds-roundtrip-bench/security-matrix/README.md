# Cross-Vendor DDS-Security Interop-Matrix Harness

Runs a security profile × 4×4 vendor matrix (cyclone/fastdds/opendds/zerodds,
both roles) for the cross-vendor interop findings. RTI left out (licence).

**Fully reproducible from any checkout** — no more `/tmp/dds-bench-security` hardcodes.
All paths are derived from the script location (can be
overridden via env: `SECDIR`, `PROFILES_DIR`, `BUILD`, `DYLIB`, `INI`).

## Prerequisites (Linux with the vendor stack, e.g. x86 host)
- Vendor round-trip binaries in `../build-sec/` (cmake; see `../CMakeLists.txt`,
  `-DOPENDDS_ROOT=/opt/opendds-secure -DCYCLONE_ROOT=/opt/cyclone -DFASTDDS_ROOT=/opt/fastdds`).
- ZeroDDS secure dylib in `<repo>/target/release/libzerodds.so`
  (`cargo build --release -p zerodds-c-api --features security`).
- The cert tree is generated automatically via `../security/gen.sh` (gitignored).
- `../security/permissions_{ping,pong}.xml` (tracked in the repo).

## Usage — one command, full matrix
```
bash run_deep_matrix.sh [SAMPLES]    # certs + all 15 profiles + 4x4 matrix
```
Or manually:
```
bash ../security/gen.sh                # cert tree (once)
#   gen_profile.sh <name> <disc> <liv> <rtps> <meta> <data> <join_ac> <rw_ac> <en_disc> <en_liv>
bash gen_profile.sh data-enc NONE NONE NONE NONE ENCRYPT false false false false
bash deep_matrix.sh 500 data-enc meta-data-enc disc-meta-data ...
```
The 15 profile definitions are in `run_deep_matrix.sh` (table) and in
`internal/security/cross-vendor-secure-interop-matrix.md` §1. Generated assets
(`profiles/`, certs, `*.p7s`, cyclone XMLs) are gitignored.

## Hard lessons (otherwise SEC_FAIL with EVERY vendor) — see
`internal/security/cross-vendor-matrix-findings.md`
- **CMS signer = permissions CA DIRECTLY** (`-signer permissions_ca.pem`), NOT
  an EE "authority" cert → otherwise cyclone `PKCS7_get0_signers: signer certificate
  not found`. (The `regen_certs.sh` used here signs WRONGLY with authority;
  `gen_profile.sh` corrects this to CA-direct.)
- **CMS MIME:** `openssl smime -sign -text` (PKCS7_TEXT) for **all** tested
  vendors incl. OpenDDS (empirical: OpenDDS of this version accepts `-text`;
  the old "opendds needs raw" finding is inverted for the bench version).
  `gen_profile.sh` still creates `text/` + `raw/`, opendds currently uses `text/`.
- governance XSD `…/20170901/…`, strict `<xs:sequence>` order, indented.
- key_agreement: ECDH+prime256v1-CEUM (spec default; X25519 is non-spec/opt-in).

## Result classification (deep_matrix.sh)
`p50=…us` (match + round trip) | `SEC_FAIL` (auth/permissions/governance) |
`NO_MATCH` (discovery/handshake timeout, pub=0/sub=0) | `FAIL` (anything else).

## Setting for Fast DDS

The column with Fast DDS runs with `ZERODDS_SEC_ALGO_NAME_NUL=mirror` in the
ZeroDDS process (`deep_matrix.sh`, `start_proc`, if the peer is `fastdds`);
the other columns run with the defaults. Fast DDS writes `c.dsign_algo`
and `c.kagree_algo` without NUL and compares the returned names;
DDS-Security 1.2 §7.3.3.1 requires the NUL. ZeroDDS answers in the form of the
request only under this setting; the default is the form of the spec.

## Setting for Cyclone DDS, Fast DDS and OpenDDS

The columns with Cyclone DDS, Fast DDS and OpenDDS run with
`ZERODDS_SEC_ALGORITHM_NAMES=1.1` in the ZeroDDS process (`deep_matrix.sh`,
`start_proc`, if the peer is `cyclone`, `fastdds` or `opendds`).
DDS-Security 1.2 Table 25 and Table 26 name `ECDSA+P256+SHA256`,
`ECDHE-CEUM+P256` and `DHE+MODP-2048-256` and mark `ECDSA-SHA256`,
`ECDH+prime256v1-CEUM` and `DH+MODP-2048-256` as "deprecated in v 1.2". ZeroDDS
sends the names of the tables by default; the three vendors report
`DDS:Auth:PKI-DH:1.0` and compare with the names of 1.1 (read from the sources,
not measured). The setting sends the names of 1.1; both spellings are
accepted on receipt. The ZeroDDS-ZeroDDS column runs with the default.

## Setting for Fast DDS and OpenDDS: form of the subject names

`dds.cert.sn`, `dds.ca.sn` and `dds.perm_ca.sn` carry the subject name;
DDS-Security 1.2 §9.4.1.2.3 specifies it as RFC 4514, which is the default of
ZeroDDS. Fast DDS 3.6 compares a received `dds.perm_ca.sn` with
`X509_NAME_oneline` of its own permissions CA (`Permissions.cpp:1007`),
OpenDDS a received `dds.ca.sn` with the string of
`X509_NAME_print_ex(.., XN_FLAG_ONELINE)` of its CA
(`AuthenticationBuiltInImpl.cpp:259`; read from the sources). The column with
Fast DDS runs with `ZERODDS_SEC_SUBJECT_NAME_FORM=x509_name_oneline`, the one with
OpenDDS with `ZERODDS_SEC_SUBJECT_NAME_FORM=xn_flag_oneline` in the ZeroDDS process
(`deep_matrix.sh`, `start_proc`). Cyclone DDS writes `X509_NAME_oneline`;
whether it compares foreign values is not known; its column runs with the
default. The ZeroDDS-ZeroDDS column runs with the default; ZeroDDS does not compare
a received `dds.ca.sn` or `dds.perm_ca.sn`.

## Profiles with origin authentication

`oa-sign` and `oa-enc` set `discovery_protection_kind`,
`liveliness_protection_kind`, `rtps_protection_kind` and
`metadata_protection_kind` to `SIGN_WITH_ORIGIN_AUTHENTICATION` and
`ENCRYPT_WITH_ORIGIN_AUTHENTICATION` respectively (DDS-Security 1.2 §10.4.1.2.5; the kind
is in `ProtectionKind` of the XSD). `data_protection_kind` is a
`BasicProtectionKind` and stays `ENCRYPT`.

Measured in round 14 (`b798333223b`, n=200 per cell, p50):

| Pair | oa-sign | oa-enc |
|---|---|---|
| Cyclone DDS -> ZeroDDS | 90.441us | 117.422us |
| ZeroDDS -> Cyclone DDS | 92.676us | 174.961us |
| Fast DDS -> ZeroDDS | 144.554us | 197.294us |
| ZeroDDS -> Fast DDS | 148.05us | 257.448us |
| OpenDDS -> OpenDDS | 212.993us | 1866.84us |
| OpenDDS -> ZeroDDS | NO_MATCH | NO_MATCH |
| ZeroDDS -> OpenDDS | NO_MATCH | NO_MATCH |
| ZeroDDS -> ZeroDDS | 96.844us | 50.826us |

By default, ZeroDDS rejects an RTPS message or submessage under a
key without a receiver-specific key if the governance requires origin
authentication (Table 80 `decode_rtps_message`: "If the kind is not
the expected one, the operation shall fail"). The cells with Cyclone DDS and
Fast DDS are green under this default; so both send receiver-specific
keys (derived, not read in their sources).

OpenDDS 3.34 generates none (`CryptoBuiltInImpl.cpp`: "origin auth is not
implemented", `receiver_specific_key_id` 0; read in the sources of
3.35.0-dev, measured on 3.34.0). In the pair with ZeroDDS, ZeroDDS rejected every
message from OpenDDS: 52 events `security.inbound.refused` in one
run of the cell, reason "the transformation_kind is not the one expected (origin
authentication)". OpenDDS -> OpenDDS is green, because neither side expects a
receiver-specific key.

`deep_matrix.sh` (`start_proc`) sets, for the ZeroDDS processes of the
OpenDDS pairs, `ZERODDS_SEC_ORIGIN_AUTHENTICATION_FALLBACK=accept` (setting
`security.origin_authentication_fallback`): a key without a
receiver-specific key is then checked with the common MAC alone,
as Table 80 describes it for `receiver_specific_key_id` 0. A
key with a receiver-specific key remains bound to it. The
default of ZeroDDS remains the rejection.

Measured with the setting (`libzerodds.so` from `893d8ee7e71`, domain 229,
n=200 per cell, p50; the other binaries from round 14):

| Pair | oa-sign | oa-enc | common-subset | meta-sign-data |
|---|---|---|---|---|
| ZeroDDS -> OpenDDS | 900.124us | 121.43us | 628.11us | 362.817us |
| OpenDDS -> ZeroDDS | 142.651us | 180.843us | 125.197us | 141.358us |
| OpenDDS -> OpenDDS | 160.354us | 219.125us | 192.294us | 207.292us |

`common-subset` and `meta-sign-data` served as a cross-check: in round 14
ZeroDDS -> OpenDDS 630.214us and 735.353us, OpenDDS -> ZeroDDS 109.227us and
178.168us.

## Empty ACKNACK of the vendors

Fast DDS (and, as recorded, OpenDDS and RTI Connext) send the ACKNACK of a reader that
holds nothing with `bitmapBase` 0 and `numBits` 0, which DDSI-RTPS 2.5 §8.3.5.5 makes
invalid. ZeroDDS by default drops the rest of such a datagram (§8.3.4.1 rule 6), and with
it the SEDP data that follows in it (measured on the Linux test host, sec-diag16/17: every red
zerodds->fastdds cell had rejections with `left_out` > 0, every green cell none; with the
setting all 24 cells green and no rejection). `deep_matrix.sh` (`start_proc`) sets
`ZERODDS_WIRE_ACCEPT_EMPTY_ACKNACK_BASE_ZERO=1` (setting
`wire.accept_empty_acknack_base_zero`) for the ZeroDDS processes of the vendor pairs, in
one place (`start_proc`). The default of ZeroDDS stays the reading of the spec.

## PL_CDR1 parameters with the rounded length

Cyclone DDS (extended header) and RTI Connext (short header) write the length of a PL_CDR1
parameter rounded up to four, the form of DDSI-RTPS 2.5 §9.4.2.11 (recorded rows;
`ast-layout` `Dialect::Cyclone` and `Dialect::Rti`, `pads_declared_length`). XTypes 1.3
§7.4.1.2.2 has the exact length, and the default reader of ZeroDDS refuses the other form.
`deep_matrix.sh` (`start_proc`) sets `ZERODDS_WIRE_PL_CDR1_READING=tolerant` (setting `wire.pl_cdr1_reading`) for the ZeroDDS process of a pair with Cyclone DDS or RTI,
in the same place as the empty-ACKNACK setting. Fast DDS and OpenDDS write the exact
length. The default of ZeroDDS stays the reading of the spec.
