# Per-Endpoint-Crypto (DDS-Security §8.5/§9.5.3.3) — Phase 3 (Receive-Seite)

**Status:** partial (Phase 1+2 DONE + verifiziert; Phase 3 offen, blocking für
cross-vendor secured discovery=ENCRYPT gegen Cyclone).
**Datum:** 2026-06-02. **Kontext:** Cross-Vendor-Security-Matrix, gate #23
(cyclone Writer-Match unter `discovery_protection_kind=ENCRYPT`).

## Was DONE ist (Phase 1+2)

ZeroDDS hatte **keine** per-Endpoint-Crypto: ein flacher Participant-Key für
ALLE Endpoints + ein key_id-Token-Dump. Das ist der „fail-first"-Antipattern
(empfange-scheitere-hoffe-auf-Re-Send), den mature Stacks (Cyclone/FastDDS)
nicht haben.

- **Phase 1** (gate, `security-runtime/src/shared.rs`): `register_local_endpoint`,
  `create_endpoint_token`, `install_remote_endpoint_token`,
  `encode_data_datawriter_by_handle`. TDD `per_endpoint_datawriter_token_
  roundtrips_by_key_id`.
- **Phase 2** (`dcps/src/runtime.rs`): Seiten-Map `endpoint_crypto`
  (`EntityId→CryptoHandle`) + `local_endpoint_crypto_handle`;
  `prepare_endpoint_crypto_tokens` sendet das **per-Endpoint**-KeyMaterial je
  `source_eid` (statt Participant-Key für alle); `protect_user_datagram`
  encodet mit dem per-Endpoint-Handle (`writer_eid_in_submessage`).

dcps-lib 453 + security-runtime 222 grün (in-process secured A↔B mit echten
per-Endpoint-Keys). Architektur jetzt spec-konform: pro Endpoint eigene key_id.

## Phase 3 — Fortschritt + Reststand (2026-06-02)

**GEFIXT (live verifiziert):**
- `install_crypto_token` nahm nur `message_data.first()` — cyclone packt aber
  ALLE per-Endpoint-Tokens (key_id 1–9) in EINE message_data-Sequenz. Fix:
  iterieren. Live: `remote_by_key_id` enthaelt jetzt key_id 1–9 (vorher nur 1),
  secure-SEDP-Decode gelingt (`open_cyclone` ~130×, vorher 0). Commit a3137654.
- secure-SEDP-Reader-ACKNACK wird jetzt per-Endpoint geschuetzt
  (`encode_datareader_submessage`, §8.4.2.4) statt clear. Commit 2bfe5abc.

### gate #23 (writer wait_for_matched) — root-caused + GEFIXT 2026-06-02

Zwei unabhaengige Root-Cause-Bugs, beide TDD-gefixt + live verifiziert (gate
schliesst: kein `wait_for_matched timeout` mehr, wenn die secure-SEDP-HBs
fliessen):

1. **rtps `add_writer_proxy` verwarf Reliability-State bei Re-Discovery**
   (Commit 233fbee4). Cyclones periodisches SPDP-Re-Announce (~3s) ersetzte den
   WriterProxy durch frischen State (`last_available=0`) → nach einem schon
   verarbeiteten HEARTBEAT meldete ZeroDDS faelschlich "nichts fehlt" (leeres
   ACKNACK `base/0`) → cyclone (min-ack 1!) liefert die SubscriptionData nie.
   Fix: bekannte GUID bewahrt SN-State, nur Locators auffrischen.

2. **security-crypto Handle-Allokator-Off-by-one** (Commit 4c802981).
   `set_remote_participant_crypto_tokens` zog `fetch_add(1)` OHNE +1,
   `insert()` mit +1 — selber Counter. Ein Remote-Token-Install kollidierte
   deterministisch mit dem zuletzt vergebenen lokalen Endpoint-Handle (der
   secure-sub-Reader ff0004c7) und ueberschrieb dessen KeyMaterial. Folge:
   ZeroDDS encodete seine ff0004c7-ACKNACK mit fremdem `transformation_key_id=1`
   (statt zufaellig) → cyclone konnte die key_id nicht zuordnen + verwarf den
   NACK still → SubscriptionData nie geliefert. Fix: beide Pfade ueber
   `next_id()`. Diagnose via SEC_PREFIX-key_id-Dump: ff0003c7=`5bb5d75f` (ok)
   vs ff0004c7=`00000001` (clobbered).

**NOCH OFFEN (2 Reste):**
1. **Flakiger secure-SEDP-HB-Fluss (~50% der Laeufe T-HB=0) — Wurzel
   geklaert 2026-06-02:** cyclone schickt seine 9 per-Endpoint-Crypto-Tokens
   ueber die VolatileSecure-Topic (reliable, durability=VOLATILE). In Bad-Runs
   geht EINE Token-Sample (z.B. seq 2/3 von 7) verloren — fruehes Kx-Decode-Fail
   (Volatile-DATA trifft ein, bevor ZeroDDS' Kx-Key aus dem Handshake bereit
   ist) oder Burst-Loss. ZeroDDS' Volatile-Reader erkennt die Luecke korrekt
   (`last_available=7, has_missing=true`) und NACKt sie 50×; cyclone EMPFAENGT
   die NACKs, aber sein VOLATILE-Writer rexmittet die Sample nicht
   (`writer_hbcontrol_p2p … seq 7 maxseq 0`, nur HB-Schleife) → ZeroDDS' in-
   order-Delivery staut permanent → nur 1-2 von 9 Tokens installiert → der
   Grossteil des secure-SEDP-Verkehrs bleibt unentschluesselbar (T-HB=0). Da
   cyclones VOLATILE-Writer nicht zuverlaessig rexmittet, MUSS ZeroDDS' Kx-Key
   **vor** cyclones erstem Token-Send bereit sein (keys-before-data, kein
   Sample-Loss). Siehe Task #28.

   **Cross-Check FastDDS-Source** (`/root/vendors/fast-dds`,
   `SecurityManager.cpp`): der ParticipantVolatileMessageSecure-Writer ist
   ebenfalls `RELIABLE + VOLATILE` (StatefulWriter), ABER mit retaining
   `WriterHistory` (`HistoryAttributes{ initialReserved=10, max=0 }`) — ein
   reliable StatefulWriter haelt unacked Changes fuer Rexmit (durability regelt
   nur Late-Joiner). FastDDS rexmittet also auf NACK; cyclone (per-SN-Zaehlung
   in der Trace: jede Volatile-SN genau 1× gesendet, `avail-seq 0`) offenbar
   nicht. → Erwartung WAR: ZeroDDS↔FastDDS laeuft durch.

   **Live-Test widerlegt die Erwartung (2026-06-02):** ZeroDDS-ping ↔
   FastDDS-pong secured (gleiche governance/certs) timeoutet DETERMINISTISCH
   (5/5). FastDDS meldet "matched", aber ZeroDDS empfaengt von FastDDS gar
   keinen Security-Traffic (T-INST=0, T-HB=0) — scheitert also FRUEHER als der
   cyclone-Pfad (kein Token-Austausch, keine secure-SEDP). Die FastDDS-Referenz
   isoliert den cyclone-Volatile-Punkt damit NICHT; ZeroDDS↔FastDDS hat einen
   eigenen, frueheren Security-Gap (Task #30). cyclone bleibt der am weitesten
   fortgeschrittene Pfad. (Anm.: cyclone-pong segfaultete in einem Lauf —
   separates Upstream-Thema.)
2. **User-DATA-Roundtrip ("no samples"):** nach erfolgreichem Match liefert der
   ping/pong-Austausch keine Samples — per-Endpoint-Crypto fuer die USER-
   Endpoints (cyclones Reader-Token fuer ZeroDDS' ping-Writer + Decode der
   pong-Replies) noch zu schliessen.

## Warum offen / Trade-off

Phase 2 (per-Endpoint **Encode/Token-Send**) ist die korrekte Architektur und
nötig (Gegenrichtung: cyclone decodiert ZeroDDS), aber adressiert NICHT die
Receive-Seite. Phase 3 ist eine eigene Reliability-Frage mit Live-Cyclone-
Iteration.

## Implikationen wenn nicht implementiert

Cross-vendor secured **Discovery-Protection (`ENCRYPT`)** gegen Cyclone/FastDDS
funktioniert nicht (User-Endpoint-Match scheitert). discovery=NONE secured
(plaintext-SEDP, encrypted DATA) ist unberührt.

## Implementations-Pfad (Phase 3, ~1–2 PT)

Deterministisch keys-vor-daten herstellen — KEIN fail-first:

1. Sicherstellen, dass ZeroDDS' secure-`SedpStack`-Reader einen **WriterProxy**
   für cyclones secure-SEDP-Writer (`ff0003c2`/`ff0004c2`) hat → NACK-fähig.
2. Undecodierbare secure-SEDP-DATA in `recv_metatraffic_loop`
   (`unprotect_user_datagram` → `None` → `sedp_input` = SEC_*-Bytes) **nicht**
   als empfangen markieren (kein SN-Advance) → der reliable Re-Send nach
   Token-Install decodet + triggert `wire_writer_to_remote_reader`.
3. Alternativ/ergänzend: nach `install_crypto_token` ein Re-Eval der pending
   secure-SEDP anstoßen.

Verifikation: cyclone secured Zelle (`cyclone pong ↔ zerodds ping`, domain 200,
`/root/bench-security`, `discovery=ENCRYPT`) → p50 statt timeout.

## Pick-up-Trigger

Cross-vendor secured discovery=ENCRYPT erforderlich (Matrix-Closeout) — direkt
fortsetzen; Phase 1+2 sind die Voraussetzung (done).
