# DDS-Security Modi — Closeout + Spec-Gegenprüfung

**Stand:** 2026-06-04. **Scope:** systematischer Abschluss der Security-Themen —
nicht nur die Common-Auswahl, sondern alle Optionen/Modi, primär **zero↔zero**
(alle Kombinationen müssen gehen), sekundär cross-vendor (Konflikte erlaubt).

## Konfigurations-Oberfläche (vollständig)

| Achse | Werte | Spec |
|---|---|---|
| ProtectionKind (je Kategorie) | NONE / SIGN / ENCRYPT / SIGN_WITH_ORIGIN_AUTH / ENCRYPT_WITH_ORIGIN_AUTH | DDS-Sec 1.2 §7.4.1 |
| Kategorie: discovery_protection | secure-SEDP (DCPSPublications/SubscriptionsSecure) | §8.4.2.4 |
| Kategorie: liveliness_protection | secure-ParticipantMessage | §8.4.2.4 |
| Kategorie: rtps_protection | message-level SRTPS (ganze RTPS-Message) | §7.3.7 / §9.5.3.3.4 |
| Kategorie: metadata_protection | per-Submessage (DATA/HEARTBEAT/GAP …) | §8.4.2.4 / §9.5.3.3.2 |
| Kategorie: data_protection | per-Endpoint SerializedPayload/DATA, Writer-Key | §9.5.3.3.1 |
| Crypto-Suite | AES128-GCM / AES256-GCM (ENCRYPT) · AES128-GMAC / AES256-GMAC (SIGN-only) | §9.5.2.1.1 |
| Auth | PKI (X.509 + ECDH-prime256v1, `DDS:Auth:PKI-DH:1.0`) · PSK (`DDS:Auth:PSK:1.2`) | §9.3 / §10.7–10.9 |
| Access-Control | Permissions (CMS/PKCS#7) · PSK-Permissions · NoOp | §9.4 / §10.8 |
| DataRepresentation | XCDR1 (0x0001) · XCDR2 (0x0007/09/0b) | XTypes 1.3 §7.6.3 |

## zero↔zero Matrix — VOLLSTÄNDIG GRÜN

Verifiziert live über UDP (Linux/codepit), `crates/dcps/tests/security_matrix_e2e.rs`,
je Combo eigene Domain, voller Handshake + Crypto-Token-Austausch + User-DATA-Roundtrip:

| Dimension | Abdeckung | Ergebnis |
|---|---|---|
| Crypto-Suites | Aes128/256-GCM × ENCRYPT **und** SIGN(GMAC) | ✅ |
| data_protection | alle 5 Kinds | ✅ |
| metadata_protection | alle 5 Kinds | ✅ |
| liveliness_protection | NONE/SIGN/ENCRYPT | ✅ |
| discovery_protection | NONE/ENCRYPT × data NONE/ENCRYPT (alle 4) | ✅ |
| **volles secure-Profil** | discovery+metadata+data=ENCRYPT **und** data=ENCRYPT/metadata=NONE | ✅ |
| Auth | PKI **und** PSK | ✅ |
| DataRep | XCDR1 **und** XCDR2 | ✅ |

**8/8 Dimensions-Tests grün, 0 ignored.** Alle zero↔zero-Kombis funktionieren.

### Schicht-Wahl-Fix (3-Wege-Dispatch, §8.4.2.4 / §9.5.3.3 / §7.3.7)

`secure_outbound_for_target` wählt jetzt spec-korrekt:
1. `metadata_protection != NONE` → per-Submessage (`protect_user_datagram`, Writer-Key)
2. sonst `rtps_protection != NONE` → message-level SRTPS (`transform_outbound_for`)
3. sonst (nur `data_protection`) → per-Endpoint-DATA (`protect_user_datagram`, Writer-Key)

Vorher wurde bei `metadata=NONE` pauschal message-level (Participant-Key,
transformation_key_id=0) genutzt; der Reader fand key_id=0 unter secure-SEDP nicht
(tag mismatch). Damit ist `data_protection` immer der per-Endpoint-Writer-Key (vom
Reader via `datawriter_crypto_tokens` installiert), nie der Participant-Fallback.

### PSK-Live-Fix

PSK implementierte die Trait-Methode `get_identity_token` nicht (Default leer) →
kein PID_IDENTITY_TOKEN im SPDP-Beacon → Peer startete den Handshake nie. Jetzt an
`build_identity_token` delegiert; PSK-Live-Handshake zero↔zero grün.

## Spec-Gegenprüfung (Konformität)

- **Suite-IDs** `AES128_GCM=0x02 / AES256_GCM=0x04 / AES128_GMAC=0x01 / AES256_GMAC=0x03`
  (crypto_transform.rs) — §9.5.2.1.1 Tab.
- **GMAC = sign-only** (`Suite::is_aead()=false` für GMAC) — SIGN-Kinds nutzen GMAC,
  ENCRYPT-Kinds GCM. Spec-konform (§8.5).
- **key_agreement Default = ECDH-prime256v1-CEUM** (nicht X25519) — Spec-vorgegeben,
  Interop schlägt Eleganz.
- **Origin-Authentication** (`*_WITH_ORIGIN_AUTHENTICATION`): Receiver-spezifische MACs
  (§9.5.3.3.4) — zero↔zero verifiziert (data/metadata SignOA/EncryptOA grün).
- **ParticipantGUID-Adjustment** (§9.3.3, OpenDDS-byte-identisch) für den
  identity-gebundenen Prefix.

## Cross-vendor (sekundär — Konflikte erlaubt)

Bench-Governance: discovery+metadata+data=ENCRYPT, rtps=NONE (sros2-Stil).

- **cyclone↔zero:** Auth-Handshake + Crypto-Token-Austausch + secure-SEDP-Match laufen
  vollständig (cyclone matcht ZeroDDS-Writer 103/Reader 204, installiert die Tokens).
  **Präziser Root (cyclone Tracing=finest):** cyclone scheitert am `decode_serialized_
  payload: Invalid syntax of encoded payload`. Die Bench-Gov hat metadata=ENCRYPT **und**
  data=ENCRYPT = **zwei Schichten** (§9.5.3.3): metadata=Submessage-Crypto (außen),
  data=**SerializedPayload-Crypto (innen, §8.5.1.9.1)**. ZeroDDS' `AesGcmCryptoPlugin`
  implementiert nur die **Submessage-Schicht** (`encrypt_submessage`), **kein**
  `encode_serialized_payload`/`decode_serialized_payload` → ZeroDDS' Payload bleibt
  Klartext, cyclone erwartet ihn verschlüsselt. zero↔zero fällt das nicht auf (ZeroDDS-
  Reader macht auch keinen Payload-Decode). **Fix-WP:** SerializedPayload-Crypto-Schicht
  implementieren + als innere Schicht wiren (Task #37). Offen (gate #23 / #29).
- **fast↔zero:** `wait_for_matched` — FastDDS tauscht über die VolatileSecure nur
  `ff0101` aus, keine `ff0003/ff0004` (secure-SEDP) → ZeroDDS' User-Endpoints werden
  nie entdeckt. FastDDS-Config/Governance-Thema, kein ZeroDDS-Bug (siehe
  `cross-vendor-interop-configs.md`).
- **cyclone↔FastDDS:** bekanntes, ungelöstes Vendor-Loch (DH-Encoding 1.1-Underspec) —
  kein ZeroDDS-Scope.
