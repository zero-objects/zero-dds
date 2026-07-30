# `zerodds-xcdr2-python` 1.0 — offene Items

**Status:** `partial` — der `idl-python`-Codegen deckt die IDL4-DataTypes (struct/enum/union/typedef/map/array/nested/bitmask/bitset/inheritance/bounded/wchar/wstring/long double/@final/@appendable/@mutable) ab; die Runtime `zerodds.cdr` marshalt + dekodiert reflektiv. Ein echtes Codegen-Item bleibt offen.

## §7.a Per-struct generierte `keyHash`-Methode aus `@key`

**Was fehlt:** `idl-python` emittiert keine per-struct `keyHash`-Methode aus `@key`-annotierten Membern. Die `zerodds`-Runtime rechnet Key-Hashes zur Laufzeit (MD5 der Key-Member in XCDR2-BE), aber generierter Codegen dafür existiert nicht.

**Warum offen (nicht rejected):** Reale Roadmap-Arbeit, cross-cutting über alle Codegen-Backends. Kein Nicht-Ziel — Key-Hash-Codegen spart den Runtime-Reflektionspfad und macht Keyed-Topics ohne Runtime-Abhängigkeit nutzbar.

**Follow-up:** Teil des Full-Clients-/Codegen-Roadmap-Programms (`internal/roadmap/full-clients-plan.md`, Feature 10).

## Echte Nicht-Ziele (kein open)

`interface` / `valuetype` / `any` → `IdlPythonError::Unsupported`. RPC/OO/dynamic-Konstrukte, keine DDS-DataTypes; bewusst außerhalb einer DataType-Wire-Binding. Belegt: `smoke.rs::interface_and_valuetype_still_unsupported`, `any_type_still_unsupported`.
