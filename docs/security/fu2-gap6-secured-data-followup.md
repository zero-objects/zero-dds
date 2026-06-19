# FU2 Gap 6 — Secured DATA: Kx-geschützter Crypto-Token-Austausch

**Status:** offen (Design-Review erforderlich vor Implementierung).
**Vorgelagert (alle DONE + verifiziert):** Gap 1–5, 7 — der **authentifizierte
PKI-Handshake** läuft end-to-end im Runtime (in-process e2e
`handshake_completes_through_runtime_dispatch_e2e`) und ist über die C-API
(`zerodds_runtime_create_secure`) erreichbar. Beide Peers leiten ein
gemeinsames 32-Byte-SharedSecret ab.

**Was Gap 6 liefert:** verschlüsselte *User-DATA* zwischen zwei authentifizierten
ZeroDDS-Participants — der letzte Schritt zur „secured"-Aussage.

---

## Kern-Befund: Dual-Key ist zwingend

Der Daten-Pfad des Crypto-Gates ist **token-basiert**:

- `transform_outbound`/`transform_outbound_for` verschlüsseln mit dem **lokalen**
  Participant-Key (der `peer_key` wird heute ignoriert).
- `transform_inbound_from` entschlüsselt mit dem **per-peer-Slot-Key**, der via
  `set_remote_participant_crypto_tokens` aus dem empfangenen Crypto-Token gesetzt
  wird.
- Round-trip funktioniert also nur, wenn der Empfänger den **lokalen Key des
  Senders** als Slot-Key kennt → er muss via Crypto-Token ausgetauscht werden.

Die Crypto-Token dürfen **nicht im Klartext** über `DCPSParticipantVolatile-
MessageSecure` laufen (sie SIND die Daten-Keys). Der Kanal muss mit einem aus dem
Handshake-Secret abgeleiteten **Kx-Key** verschlüsselt sein.

**Warum Single-Slot nicht reicht:** Würde man denselben Peer-Slot erst mit dem
Kx-Key (aus Secret) und dann via `set_remote_participant_crypto_tokens` mit dem
Daten-Key überschreiben, bricht der **bidirektionale** Token-Austausch: sobald
Seite A den Token von B verarbeitet (Slot → A-sieht-B-Daten-Key), kann A seinen
eigenen Token nicht mehr Kx-verschlüsselt senden. Beide Seiten brauchen den Kx-Key,
bis **beide** ausgetauscht haben. ⇒ pro Peer werden **zwei** Schlüssel parallel
gebraucht: `kx_key` (VolatileSecure-Schutz) + `data_key` (User-DATA).

---

## Zwei Implementierungs-Ansätze (Entscheidung offen)

### Ansatz A — Dual-Key im Crypto-Plugin + Gate (spec-treu)

`AesGcmCryptoPlugin::KeyMaterial` bekommt zwei Key-Felder: `kx_key` (aus
`from_shared_secret`) und `data_key` (aus Token / lokal). `encode/decode_submessage`
wählt anhand des Topics (VolatileSecure → kx, sonst → data). Das Gate hält pro Peer
**einen** Crypto-Handle (wie Spec §9.5: ein `ParticipantCrypto` mit
`KxKeyMaterial` + `ParticipantKeyMaterial`).

- **+** spec-getreu (§9.5.3), näher an FastDDS/Cyclone-Modell → bessere Cross-
  Vendor-Basis.
- **−** invasiver Umbau des 1298-Zeilen-Plugins (KeyMaterial, encode/decode,
  serialize, alle Tests).

### Ansatz B — Separater Kx-Layer im Stack (minimal-invasiv)

Der `SecurityBuiltinStack` (der das Secret nach Handshake bereits hält) ver-/
entschlüsselt die VolatileSecure-Payloads selbst mit einem aus dem Secret
abgeleiteten Kx-Key (über die getestete AES-GCM-Primitive aus `security-crypto`).
Das Gate bleibt single-slot und hält **nur** den Daten-Key (aus Token).

- **+** Crypto-Plugin + Gate-Datenpfad bleiben unangetastet; klare Schicht-Trennung.
- **−** koppelt `discovery` an `security-crypto`; Kx-Schicht lebt außerhalb des
  Plugin-SPI (weniger spec-formal).

**Empfehlung:** Ansatz A, weil die Cross-Vendor-Zielsetzung (FU2) ohnehin das
spec-Modell verlangt und Ansatz B beim späteren Cross-Vendor-Schritt wieder
umgebaut werden müsste. Bei Zeitdruck ist B ein valider Zwischenschritt.

---

## Token-Austausch-Fluss (nach Handshake-Complete, beide Ansätze)

1. **Complete** (Gap 5 liefert `(remote_identity, shared_secret)` pro Peer).
2. Kx registrieren: `register_matched_remote_participant(local, remote_id,
   shared_secret)` → Kx-Key (deterministisch, beide Seiten gleich).
3. Eigenen Token senden: `gate.local_token()` →
   `volatile_writer.write(PARTICIPANT_CRYPTO_TOKENS)`, **Kx-verschlüsselt**.
4. Peer-Token empfangen (dispatch, VolatileSecure-Reader, **Kx-entschlüsselt**) →
   `set_remote_participant_crypto_tokens` / `register_remote_by_guid` → Daten-Key.
5. Ab jetzt: `transform_outbound` (lokaler Daten-Key) ↔ `transform_inbound_from`
   (Peer-Daten-Key) → secured DATA round-trippt.

**Anknüpfpunkte:** `dispatch_security_builtin_datagram` (runtime.rs) hat den
VolatileSecure-Reader-Pfad bereits; die Completion aus `on_stateless_message`
(heute verworfen) triggert Schritt 2–3. `discovery/security/volatile_secure.rs`
ist heute **plaintext** (0 Crypto-Referenzen) → Schritt 3/4-Encryption fehlt.

---

## Verifizierbarkeit

In-Process testbar (kein Linux/Multicast nötig): Token-Austausch über
VolatileSecure zwischen zwei Stacks pumpen (analog zum Stateless-Pump in
`handshake_completes_through_runtime_dispatch_e2e`), dann via Gate ein User-DATA-
Datagramm A→B `transform_outbound` → `transform_inbound_from` round-trippen und
Klartext-Gleichheit asserten. **Erst grün mergen.**

## Cross-Vendor (separater, größerer Task)

Secured DATA gegen FastDDS/Cyclone verlangt **exaktes** Matching von Crypto-Token-
Format, KDF (`master_salt`/`session_id`/`key_id`-Ableitung) und SEC_PREFIX/
POSTFIX-Layout — iterative Live-Arbeit auf codepit mit Wire-Byte-Vergleich. Nicht
Teil von Gap 6 (ZeroDDS↔ZeroDDS), sondern Folge-Phase.
