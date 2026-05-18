# DDS-DCPS 1.4 — Open Items

## §2.1.3 DLRL-Layer

**Status:** `open` — Optionaler Data-Local-Reconstruction-Layer ueber
DCPS. ZeroDDS implementiert ihn als Differenzierungs-Feature: keiner
der Major-Vendoren (RTI/Cyclone/FastDDS) hat DLRL — exakt deshalb
ist es ein Sales-Argument fuer Bestands-Migration aus den
DDS-Implementations der frueheren Generationen, die DLRL erwartet
haben.

**Plan-Hinweis:** WP DDS-DLRL-Layer (~13-17 PW). Bestandteile:
- Object-Cache mit Identity-Tracking — 2-3 PW
- Relationship-Resolver (1:1/1:N/N:M) — 3-4 PW
- Transaktions-Semantik mit optimistic-locking — 3-4 PW
- Sample-Mapping (DCPS-Sample ↔ DLRL-Object) — 2 PW
- Inline-QoS-Erweiterung fuer Object-Identity — 1 PW
- Conformance-Tests + Audit — 2-3 PW

Eigener Crate `crates/dlrl/` auf DCPS-Public-API. Phase-3-Block,
parallel zu CORBA-/CCM-WP.

## §2.3.1-2 PIM→PSM CORBA-IDL-Annex-A.1-Emission

**Status:** done — Annex-A.1-Codegen-Pfad ist in
`crates/idl-cpp/src/corba_traits.rs`,
`crates/idl-csharp/src/corba_traits.rs`,
`crates/idl-java/src/corba_traits.rs` als opt-in Backend live
(siehe `idl4-cpp-1.0.md` / `idl4-csharp-1.0.md` / `idl4-java-1.0.md`
Annex A.1). Wire-Backend (GIOP/IIOP/POA/CSIv2) bleibt separat
in `corba-3.3.md`.
