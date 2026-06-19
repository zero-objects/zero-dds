# Vendor-Matrix — Provider-Research

Ziel: jede Wettbewerber-Aussage in `website/topics/vendor-matrix/index.html`
gegen **echte, abgerufene Quellen** verifizieren. Keine erfundenen Links/Claims.
Pro Provider: aktuelle Version, Matrix-Pin, Status der Capability-Claims, Quelle.

Legende: ✅ Claim bestätigt · ✏️ Versions-Pin stale (Substanz ok) · ⚠️ Claim
womöglich überholt → Primärquelle nötig · ❌ Claim falsch.

---

## A. DDS-Hauptvendoren

### A.1 Eclipse Cyclone DDS
- **Aktuelle Version: 11.0.1** (Doc-Stand). Matrix-Pin „0.10.x" → ✏️ **stark
  veraltet** (Cyclone ist auf Kalender-/Major-Versionierung 11.x gesprungen).
  Quelle: <https://cyclonedds.io/docs/cyclonedds/latest/> (Header „11.0.1");
  Releases <https://github.com/eclipse-cyclonedds/cyclonedds/releases>.
- **XTypes-TypeLookup-Dependency-Limit (Matrix ⁽ᶠ⁾ §2.2): ✅ inhaltlich korrekt,
  gilt weiterhin im master.** Zitat: „The built-in TypeLookup service (7.6.3.3)
  has no support for requesting type dependencies (getTypeDependencies …)" und
  „handling PublicationBuiltinTopicData … with an incomplete set of dependent
  types … may result in a failure to match a reader with a writer."
  Zusätzlich NICHT unterstützt: Map, bitset, wide-strings, char16, float128,
  Union-Inheritance + mutable extensibility, Dynamic-Language-Binding-API,
  rekursive Typen im TypeObject, XML/XSD-Repräsentation.
  Quelle: <https://github.com/eclipse-cyclonedds/cyclonedds/blob/master/docs/dev/xtypes_relnotes.md>.
  → **Aktion:** Versions-Pin „0.10.x" auf „11.x" heben, Substanz behalten.
- DataTagging-Security-Claim (Matrix ⁽ᵇ⁾ §2.3): ⚠️ Security-Docs-Seite noch
  primär zu prüfen (Landing-Page bestätigt nur, dass es eine Security-Sektion
  gibt). Quelle: <https://cyclonedds.io/docs/cyclonedds/latest/>.

### A.2 eProsima Fast-DDS
- **Aktuelle Version: 3.6.1** (Doc + Releases). Matrix-Pin „3.x" → ✏️ ok aber
  unspezifisch; auf „3.6.x" präzisieren. v3.6.0 enthielt einen CVE-Fix
  (CVE-2026-22591). Quelle: <https://github.com/eProsima/Fast-DDS/releases>,
  <https://fast-dds.docs.eprosima.com/en/latest/notes/notes.html>.
- TypeObjectV2-Claim (Matrix ⁽ᵈ⁾ §2.2 „partiell, Roadmap eProsima 4.0"): ⚠️
  XTypes-Doc-Detailseite lieferte noch keine Bestätigung — Primärquelle
  (xtypes-Sektion) nötig, bevor Substanz geändert wird.

### A.3 RTI Connext DDS
- **Aktuelle Version: 7.7.0 LTS** (Q2 2026; Python/NuGet-Binding 7.7.0 seit
  2026-04-27). Matrix-Pin „7.x"/„bis 7.x" → ✏️ präzisieren auf 7.7 LTS.
  Quelle: <https://www.nuget.org/packages/Rti.ConnextDds> (7.7.0),
  <https://community.rti.com/connext-releases>.
- TypeObjectV2/TypeLookup-Claim (Matrix ⁽ᵉ⁾ §2.2 „für nächstes LTS
  angekündigt"): ⚠️ **potenziell überholt** — 7.7 LTS ist jetzt erschienen,
  „angekündigt für nächstes LTS" könnte erfüllt sein. What's-New-7.7 (oft
  hinter Doc-Portal) als Primärquelle nötig.
- Connext-Micro RTOS-only (Matrix ⁽ᵇ⁾ §3): noch gegen 7.x-Micro-Platforms
  zu prüfen.

### A.4 OpenDDS
- **Aktuelle Version: 3.34.0** (Release 2026-05-21; 3.35.0 in dev). Matrix-Pin
  „3.34.x" → ✅ **aktuell**. Quelle:
  <https://github.com/OpenDDS/OpenDDS/releases/tag/v3.34.0>,
  <https://opendds.readthedocs.io/en/master/news.html>.
- TypeLookup-Limit-Claim (Matrix ⁽ᵍ⁾ §2.2): noch gegen 3.34-Devguide zu prüfen.

### A.5 dust-dds
- Matrix: „Pre-1.0, ~11/40 QoS-Policies". Noch gegen aktuelles README/Releases
  zu verifizieren (Versions-/QoS-Zahl). Quelle (Matrix-zitiert):
  <https://github.com/s2e-systems/dust-dds#readme>.

---

## B. Foreign-Spec-Konkurrenten (§7.x)

### B.1 OPC-UA (§7.7) — Konkurrenz-Ratings korrigiert
- **opcua-rs (locka99/opcua): ❌ Matrix-Fehler.** Hat **kein** PubSub/Part 14 —
  nur `opc.tcp`-Binär-Client/Server. Matrix-Rating war ◐ ⁽ᵇ⁾ („UADP-Codec
  implementiert"). → **korrigiert auf ✗.** Quelle:
  <https://github.com/locka99/opcua/blob/master/docs/compatibility.md>
  („supports the opc.tcp:// binary protocol", keine PubSub/UADP-Erwähnung).
- **Eclipse Milo: ❌ Matrix-Fehler.** Nur Client-/Server-SDK
  (`opc-ua-sdk`/`opc-ua-stack`), **kein** PubSub/UADP (Part 14 war für „2.0"
  zurückgestellt, Issue #520). Matrix war ◐ ⁽ᵇ⁾. → **korrigiert auf ✗.**
  Quelle: <https://github.com/eclipse-milo/milo> (nur client/server SDK).
- **open62541: ✅ korrekt.** Voller PubSub/UADP (UDP-Multicast, Ethernet,
  MQTT, AMQP). Matrix ✓ stimmt. Quelle:
  <https://open62541.org/doc/master/pubsub.html>.
- Fußnote ⁽ᵇ⁾ (vm_251) komplett neu geschrieben: „Weder opcua-rs noch Milo
  implementieren Part-14-PubSub".

### B.2 AMQP (§7.1) — ✅ bestätigt
- **lapin = nur AMQP 0.9.1; fe2o3-amqp = nur AMQP 1.0** — Matrix-Claim ⁽ᵃ⁾
  korrekt. Quellen: <https://github.com/amqp-rs/lapin> („follows the AMQP
  0.9.1 specifications"), <https://github.com/minghuaw/fe2o3-amqp> („AMQP1.0
  protocol"). Qpid Proton = AMQP 1.0 (korrekt).

### B.3 MQTT (§7.2) — Konkurrenz-Ratings korrigiert
- **rust-mqtt: ❌ Matrix-Fehler.** Unterstützt **nur MQTT 5.0** (3.1.1 nur als
  künftiges Feature). Matrix zeigte ✓ für „MQTT 3.1.1 Backwards". →
  **korrigiert auf ✗ ⁽ᶜ⁾.** Quelle: <https://docs.rs/rust-mqtt/latest/rust_mqtt/>
  („As of now, only MQTT version 5.0 is supported").
- **Fußnote ⁽ᶜ⁾ (vm_208) korrigiert:** TLS ist NICHT std-gated — läuft via
  `embedded-tls` im no_std-Profil; WebSocket-Transport fehlt ganz; nur v5.
  Quelle: docs.rs (TLS-Beispiel mit embedded-tls).
- rumqtt/rumqttc (✓ 5.0+3.1.1) + Eclipse Paho (✓): nicht widerlegt, bleiben.

### B.4 DDS-XTypes-Substanz (§2.2) — verifiziert
- **RTI Connext §2.2 (vm_073 / p_fn_typelookup): ❌ Matrix-Fehler → korrigiert.**
  „TypeObjectV2/TypeLookup für nächstes LTS angekündigt" war stale. Hard
  evidence: „Connext 7.7.0 and higher supports TypeObject v1 and TypeObject v2 …
  TypeObject v2 is propagated by default"; TypeLookup-Service = 4 Builtin-
  Endpoints, holt vollen TypeObject v2 „and all of its dependencies" (seit 6.0).
  → §2.2 RTI-Zellen TypeObjectV2 + TypeLookup ◐ → **✓**; Fußnoten neu.
  Quelle: <https://community.rti.com/static/documentation/connext-dds/current/doc/manuals/connext_dds_professional/extensible_types_guide/extensible_types/Type_Representation.htm>.
- **OpenDDS §2.2 (vm_075): ◐ korrekt, bleibt.** TypeLookup + Minimal/Complete
  TypeObject vorhanden (Complete via `-Gxtypes-complete`), aber „Basic
  Conformance level, partial Dynamic Language Binding"; nicht unterstützt:
  bitmask/bitset, struct-inheritance, @external/@must_understand, maps,
  runtime type construction, MultiTopic, XCDRv1. Quelle:
  <https://opendds.readthedocs.io/en/master/devguide/xtypes.html>.
- **Fast-DDS §2.2 (vm_072): ◐ belassen** — Doc bestätigt Minimal+Complete
  TypeObject-Repräsentation + Remote-Type-Discovery, aber die fast-DDS-Seiten
  gaben keine harte Aussage zur Vollständigkeit/Dependency-Resolution; ohne
  Beleg nicht geändert. Quelle: <https://fast-dds.docs.eprosima.com/en/latest/fastdds/xtypes/xtypes.html>.
- **dust-dds: aktuell 0.15.0** (2026-03, Pre-1.0 ✓). „~11/40 QoS" nicht
  präzise gegenverifizierbar (README-Selbstangabe) → belassen. Quelle:
  <https://github.com/s2e-systems/dust-dds/releases>.

### B.5 CoAP/WebSocket (§7.3/§7.4) — geprüft, NICHT geändert (mehrdeutig)
Bewusst unverändert gelassen, weil keine airtight-Quelle (Web-Suche vermischt
`coap`/`coap-lite`/`coap-server-rs`/`libcoap-rs`):
- **⚠️ coap-rs DTLS (vm_217):** Matrix sagt „DTLS via openssl, kein rustls". Das
  `coap`-Crate nutzt jedoch laut Doc den `webrtc-rs`-DTLS-Backend (pure-Rust),
  und `coap-lite` unterstützt Block1 **und** Block2 — d.h. die Matrix-Claims
  „nur Block2/Server-Side" + „openssl" sind **möglicherweise veraltet**. Vor
  Änderung: `Covertness/coap`-Repo + `coap-lite`-Doc direkt prüfen.
  Quellen: <https://docs.rs/coap/>, <https://docs.rs/coap-lite/latest/coap_lite/block_handler/index.html>.
- **⚠️ tungstenite permessage-deflate (vm_225):** Matrix sagt „optionales
  `deflate`-Cargo-Feature". Aktueller Stand laut Issue #2/PR #235 eher „nicht im
  Stable-Release" — Claim evtl. zu großzügig (tungstenite hätte dann gar kein
  deflate, müsste ✗ statt ◐ sein). Vor Änderung: tungstenite-`Cargo.toml`-
  Features der aktuellen Version (0.29) prüfen.
  Quelle: <https://github.com/snapview/tungstenite-rs/issues/2>.
- **✅ ws-rs (vm_226):** „unmaintained, TLS via openssl/ssl-Feature" — plausibel
  bestätigt (ws 0.9.2 letzte, viele offene Issues, ssl+permessage-deflate via
  Feature). Bleibt. Quelle: <https://github.com/housleyjk/ws-rs>.

### B.6 Nicht web-verifiziert (stabile Architektur-Fakten, belassen)
- §3 Connext-Micro „RTOS-only" (vm_110): bekannter, stabiler Fakt (Connext
  Micro zielt auf VxWorks/INTEGRITY/LynxOS/FreeRTOS). Versionspin „4.x" evtl.
  prüfbar, Substanz unstrittig.
- §7.8 rmw_iceoryx „nur Zero-Copy-IPC, host-lokal, kein RTPS" (vm_260):
  architektonisch korrekt (iceoryx = Shared-Memory).
- §6 CORBA (omniORB-IR, TAO-kein-DDS, AXCIOMA-Nachfolger, Lizenzen): stabile
  Architektur-/Lizenz-Fakten; Stichprobe plausibel, kein Einzelnachweis geführt.

---

## Zusammenfassung der angewandten Korrekturen (Commits fc06d7a8 + 74c97447)
1. §7.7 opcua-rs + Milo PubSub ◐ → ✗ (kein Part-14; belegt).
2. §7.2 rust-mqtt 3.1.1 ✓ → ✗ + Fußnote (v5-only; no_std-TLS; belegt).
3. §2.2 RTI Connext TypeObjectV2 + TypeLookup ◐ → ✓ + Fußnoten (7.7 LTS hat
   beides; belegt).
4. Cyclone-Versionspins 0.9.0/0.10.x → 11.0.1 (XTypes-Limit weiter gültig).
5. §7.1 AMQP-Familien-Claim verifiziert (lapin=0.9.1, fe2o3=1.0).
6. open62541 ✓ / OpenDDS ◐ / Fast-DDS ◐ verifiziert bzw. belegt-belassen.

### C. Angewandte Matrix-Korrekturen (Commit folgt)
1. Cyclone-Versions-Pin 0.9.0/0.10.x → **11.0.1** (XTypes-Limit-Claim per
   master-`xtypes_relnotes.md` weiterhin gültig) — alle Vorkommen (vm_074,
   fe_022, p_fn_typelookup, DE+EN).
2. §7.7 opcua-rs + Milo PubSub ◐ → **✗**; Fußnote vm_251 neu.
3. §7.2 rust-mqtt 3.1.1 ✓ → **✗**; Fußnote vm_208 korrigiert (no_std-TLS, v5-only).
