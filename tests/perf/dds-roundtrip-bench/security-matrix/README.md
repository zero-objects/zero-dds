# Cross-Vendor DDS-Security Interop-Matrix Harness

Fährt eine Security-Profil × 4×4-Vendor-Matrix (cyclone/fastdds/opendds/zerodds,
beide Rollen) für die Cross-Vendor-Interop-Findings. RTI ausgelassen (Lizenz).

**Voll reproduzierbar aus jedem Checkout** — keine `/tmp/dds-bench-security`-Hardcodes
mehr. Alle Pfade werden aus der Script-Location abgeleitet (per env
ueberschreibbar: `SECDIR`, `PROFILES_DIR`, `BUILD`, `DYLIB`, `INI`).

## Voraussetzungen (Linux mit Vendor-Stack, z.B. x86-host)
- Vendor-Roundtrip-Binaries in `../build-sec/` (cmake; siehe `../CMakeLists.txt`,
  `-DOPENDDS_ROOT=/opt/opendds-secure -DCYCLONE_ROOT=/opt/cyclone -DFASTDDS_ROOT=/opt/fastdds`).
- ZeroDDS-secure-dylib in `<repo>/target/release/libzerodds.so`
  (`cargo build --release -p zerodds-c-api --features security`).
- cert-Tree wird automatisch via `../security/gen.sh` erzeugt (gitignored).
- `../security/permissions_{ping,pong}.xml` (im Repo getrackt).

## Nutzung — ein Befehl, volle Matrix
```
bash run_deep_matrix.sh [SAMPLES]    # certs + alle 13 Profile + 4x4-Matrix
```
Oder manuell:
```
bash ../security/gen.sh                # cert-Tree (einmalig)
#   gen_profile.sh <name> <disc> <liv> <rtps> <meta> <data> <join_ac> <rw_ac> <en_disc> <en_liv>
bash gen_profile.sh data-enc NONE NONE NONE NONE ENCRYPT false false false false
bash deep_matrix.sh 500 data-enc meta-data-enc disc-meta-data ...
```
Die 13 Profil-Definitionen stehen in `run_deep_matrix.sh` (Tabelle) bzw.
`internal/security/cross-vendor-secure-interop-matrix.md` §1. Generierte Assets
(`profiles/`, certs, `*.p7s`, cyclone-XMLs) sind gitignored.

## Harte Learnings (sonst SEC_FAIL bei JEDEM Vendor) — siehe
`internal/security/cross-vendor-matrix-findings.md`
- **CMS-Signer = Permissions-CA DIREKT** (`-signer permissions_ca.pem`), NICHT
  ein EE-„authority"-Cert → sonst cyclone `PKCS7_get0_signers: signer certificate
  not found`. (Das hier verwendete `regen_certs.sh` signiert FALSCH mit authority;
  `gen_profile.sh` korrigiert das auf CA-direkt.)
- **CMS-MIME:** `openssl smime -sign -text` (PKCS7_TEXT) für **alle** getesteten
  Vendoren inkl. OpenDDS (empirisch: OpenDDS dieser Version akzeptiert `-text`;
  das alte „opendds braucht raw"-Finding ist für die Bench-Version invertiert).
  `gen_profile.sh` legt trotzdem `text/` + `raw/` an, opendds nutzt aktuell `text/`.
- governance XSD `…/20170901/…`, strikte `<xs:sequence>`-Reihenfolge, eingerückt.
- key_agreement: ECDH+prime256v1-CEUM (Spec-Default; X25519 ist Nicht-Spec/opt-in).

## Ergebnis-Klassifikation (deep_matrix.sh)
`p50=…us` (Match + Roundtrip) | `SEC_FAIL` (Auth/Permissions/governance) |
`NO_MATCH` (Discovery/Handshake-Timeout, pub=0/sub=0) | `FAIL` (sonstiges).
