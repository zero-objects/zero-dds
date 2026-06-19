# FU2 Secured — Abschluss-Plan (volle 4-Vendor-Matrix)

**Ziel:** DDS-Security end-to-end abschließen — authentifizierter Handshake (✅ done),
secured DATA (ZeroDDS↔ZeroDDS), über alle Transport-Kanäle, gegen alle vier
Fremd-Vendoren (FastDDS, Cyclone, RTI, OpenDDS), über alle Protection-Levels.

**Entscheidungen (2026-05-30):**
- Gap 6 = **Ansatz A** (Dual-Key im Crypto-Plugin/Gate, spec-treu §9.5).
- Scope = **volle 4-Vendor-Matrix** (S1–S5).

**Verifikations-Disziplin (gilt durchgängig):** kein Merge ohne grünen Test;
jeder Crypto-Schritt mit echtem encrypt-A→decrypt-B-Round-trip (falscher Key ⇒
rot); keine fake-„secured"-Aussage; pro Schritt Commit + clippy/fmt.

---

## ✅ Done — authentifizierter Handshake-Layer (7 Commits, 2026-05-30)

Gap 3 (`afadf25f`) Driver · Gap 4 (`25de911d`) Runtime-with_auth · Gap 7b
(`b383bfda`) IdentityToken-Deskriptor · Gap 7c/d (`b60845ee`) SPDP-Announce+Trigger ·
Gap 5 (`a7851b04`) Dispatch+e2e · Gap 2 (`b7baa9ac`) FFI-enable · Gap 6-Doc
(`f8dd187d`). Zwei Participants vollenden den PKI-3-Runden-Handshake, gemeinsames
32-Byte-Secret, e2e in-process + über C-API.

---

## Phase S1 — Secured DATA (Dual-Key, Ansatz A) · in-process verifizierbar

Kern. Reihenfolge mit TDD + Commit pro Schritt.

- **S1.1 — Crypto-Plugin Dual-Key** (`security-crypto/src/plugin.rs`)
  `KeyMaterial` hält zwei Schlüsselsätze: `kx` (KeyExchange, schützt VolatileSecure)
  + `data` (User-DATA). `register_matched_remote_participant(secret)` füllt `kx` via
  `from_shared_secret`; `set_remote_participant_crypto_tokens(token)` füllt `data`
  (überschreibt NICHT mehr `kx`). encode/decode bekommen einen Kanal-Selektor
  (`Kx` | `Data`) — separate Methoden `encode_kx_submessage`/`encode_submessage`
  oder ein enum-Param.
  *Gate-Test:* round-trip mit kx-Key, round-trip mit data-Key, Unabhängigkeit
  (data-set überschreibt kx nicht), zwei Plugins mit gleichem Secret → gleicher
  kx-Key. Bestehende 30+ Crypto-Tests grün halten.

- **S1.2 — Gate Dual-Register** (`security-runtime/src/shared.rs` + `gate.rs`)
  `register_remote_by_guid_from_secret(peer_key, remote_id, secret)` → nur `kx`
  (kein Token-Clobber). Bestehendes `register_remote_by_guid(...token)` → `data`.
  Neue `transform_kx_outbound_for`/`transform_kx_inbound_from` (nutzen den kx-Key)
  für den VolatileSecure-Kanal; `transform_outbound(_for)`/`transform_inbound_from`
  bleiben (data-Key).
  *Gate-Test:* VolatileSecure-Payload A→B über kx round-trippt; User-DATA über data
  round-trippt; beide unabhängig.

- **S1.3 — VolatileSecure Kx-Encryption** (`discovery/security/volatile_secure.rs`
  heute plaintext + `dcps/runtime.rs` dispatch)
  Volatile-Writer-Payloads werden vor dem Wire-Encode kx-verschlüsselt, Reader
  entschlüsselt nach Decode. Hook im Dispatch (`dispatch_security_builtin_datagram`,
  Volatile-Zweig) + im Volatile-Send-Pfad (poll/write). Schlüssel = peer kx-Slot.

- **S1.4 — Token-Austausch bei Complete** (`dcps/runtime.rs` dispatch, nutzt die
  Gap-5-Completion die heute verworfen wird)
  Bei `on_stateless_message`-Complete `(remote_id, secret)`:
  (a) `gate.register_remote_by_guid_from_secret(peer_key, remote_id, secret)` → kx;
  (b) `gate.local_token()` → über `volatile_writer.write(PARTICIPANT_CRYPTO_TOKENS)`
  kx-verschlüsselt senden. Bei Empfang eines Peer-Tokens (Volatile-Reader, kx-
  entschlüsselt) → `gate.register_remote_by_guid(peer_key, remote_id, secret, token)`
  → data-Slot.

- **S1.5 — e2e Secured-DATA** (`dcps/runtime.rs` test, in-process)
  Zwei Stacks + Gates: vollen Handshake (Gap 5) → Token-Austausch (S1.4) pumpen →
  dann User-DATA A→`transform_outbound`→`transform_inbound_from`@B → Klartext-
  Gleichheit. **Das ist der secured-DATA-Beweis.**

**S1-Gate:** ZeroDDS↔ZeroDDS secured DATA round-trippt in-process. → S2.

---

## Phase S2 — Live-Verifikation (codepit / Linux)

macOS kann Multicast-Loopback nicht (`target_os=linux`-gated). Auf codepit
(`ssh root@codepit`, Debian 13):
- **S2.1** Zwei echte DcpsRuntimes (UDP-Multicast) → Live-Handshake-Complete
  (poll auf `peer_secret`, Timeout). Linux-gated Integ-Test in `dcps/tests/`.
- **S2.2** Live secured DATA round-trip (Writer→Reader, `data_protection=ENCRYPT`).
- Code-Sync auf codepit (git push interner Branch oder rsync working-tree),
  `nohup`-Build, Pipeline-Budget-Disziplin beachten.

**S2-Gate:** Live ZeroDDS↔ZeroDDS secured DATA grün.

---

## Phase S3 — Multi-Channel (Transport-Matrix)

Gate sitzt transport-agnostisch im Outbound-/Inbound-Byte-Pfad. Pro Transport nur
verifizieren (+ ggf. MTU/Frame-Eigenheiten):
UDP (Baseline ✓ aus S1/S2) · TCP · SHM (same-host-shm) · UDS (same-host-uds) ·
TSN (tsn-live, Linux). Je ein secured-DATA-Round-trip-Test pro Kanal.

**S3-Gate:** secured DATA über alle ZeroDDS-Transporte.

---

## Phase S4 — Cross-Vendor (die Matrix) · iterativ, live, codepit

Eigene Liga: jeder Vendor verlangt byte-genaues Matching von Crypto-Token-Format,
KDF (`master_salt`/`session_id`/`key_id`-Ableitung, Spec §9.5.3.3.4) und
SEC_PREFIX/BODY/POSTFIX-Layout (§9.5.2). Bench-Stack auf codepit + m1-new
(5-Vendor-Install-Reports im Repo).

- **S4.0 — Directional-Discovery-Fix** (offener FU1-Punkt, siehe
  `[[zerodds-xvendor-directional-discovery]]`): ZeroDDS-Late-Joiner findet
  cyclone/fastdds nicht (opendds geht). Vorbedingung für jede gerichtete
  Cross-Vendor-Session.
- **S4.1 FastDDS** secured interop — Leit-Vendor (codepit hat FastDDS). Token-Wire
  byte-vergleichen, iterativ angleichen.
- **S4.2 Cyclone DDS** secured interop.
- **S4.3 RTI Connext** + **OpenDDS** secured interop.

**Die Matrix** (pro Vendor-Paar, beide Richtungen):
`{discovery_protection} × {data_protection} × {None, Sign, Encrypt}` plus
Initiator/Replier-Rolle. Ergebnis = Conformance-Tabelle (✓/✗/n-a pro Zelle).

**Realismus:** unvorhersehbarer Aufwand pro Vendor (Wire-Reverse-Engineering).
Jeder Vendor ist ein eigenes Sub-Projekt mit `nohup`-Jobs; nicht parallel-blind.
Cyclone-Wire-Compliance + Live-Discovery sind bereits etabliert (WP 0.6/1.4) — das
ist die beste Startbasis nach FastDDS.

**S4-Gate:** Matrix-Tabelle vollständig, jede Zelle belegt (✓ mit Live-Beweis
oder dokumentiertes ✗/n-a mit Grund).

---

## Phase S5 — Closeout

- Conformance-Matrix in `docs/spec-coverage/` (DDS-Security 1.2) ergänzen/aktualisieren.
- `docs/OPEN-ITEMS.md`-Eintrag schließen; `fu2-gap6-*` + dieser Plan als done markieren.
- Cross-Vendor-Gotchas dokumentieren (CMS/governance/Token-Format pro Vendor).
- Memory `[[fu2-secured-handshake-wiring]]` + `[[zerodds-security-ffi-live]]` final.

---

## Kritischer Pfad & Reihenfolge

```
S1.1 → S1.2 → S1.3 → S1.4 → S1.5  (in-process, macOS)   ← Blocker für alles
                                  ↓
                          S2 (live codepit)
                          ↓        ↓
                         S3       S4.0 → S4.1 → S4.2 → S4.3
                          \________________/
                                  ↓
                                 S5
```

S1 ist der harte Kern (in-process voll verifizierbar). S2/S3 sind mechanisch sobald
S1 steht. S4 ist die zeit-dominante, unvorhersehbare Phase (Vendor-Wire-Matching).

## Risiken
- **R1 (S1):** Dual-Key-Routing-Bug rutscht durch → mitigiert durch echten
  encrypt-A→decrypt-B-Test (kein Mock).
- **R2 (S2):** Live-Multicast-Flakiness → poll+Timeout, codepit statt CI-default.
- **R3 (S4):** Vendor-Crypto-Format weicht ab und ist schlecht dokumentiert →
  Wireshark-Lua + pcap-Vergleich (PDE-Tooling vorhanden), iterativ, Zeit-offen.
