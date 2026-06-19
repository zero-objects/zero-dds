# Dossier — DTLS für die CoAP-Bridge: pure-Rust-Lage & Entscheidung

- **Status:** Entscheidungs-Dossier (informational)
- **Datum:** 2026-06-11
- **Scope:** `crates/coap-bridge` §7.1 / RFC 7252 §9 (DTLS-Security-Modi),
  Bezug zu ADR 0007/0010 (OSCORE) und [[feedback_spec_completeness_over_competition]]

## 1. Problem

RFC 7252 §9 definiert vier CoAP-Security-Modi: **NoSec**, **PreSharedKey**,
**RawPublicKey**, **Certificate**. Die drei gesicherten Modi laufen über
**DTLS** (Datagram-TLS, RFC 6347 für 1.2 / RFC 9147 für 1.3) — TLS über UDP,
nicht über TCP.

Der ZeroDDS-Workspace nutzt für TLS durchgehend **rustls** (`bridge-security`):
mTLS auf den TCP-Bridges, ServerConfig, Peer-Cert-Extraktion. **rustls
implementiert aber ausschließlich TLS 1.2/1.3 über Streams — kein DTLS** (kein
Datagram-Record-Layer, keine HelloVerifyRequest/Cookie-Maschinerie, kein
Epoch/Sequence-Number-Handling für UDP-Reordering).

**Ist-Stand ZeroDDS:** `crates/coap-bridge/src/dtls.rs` liefert die
**Konfigurations-Schicht** vollständig (`DtlsMode`-Enum für alle vier Modi +
PSK/RawPublicKey/Certificate-Identity-Strukturen, gebaut auf `security-pki` +
`security-crypto`). Die **DTLS-Record-Layer selbst fehlt** — der Daemon wired
Auth + ACL voll und meldet bei gesetztem `--tls-cert/--tls-key` einen klaren
„DTLS-Wireup via separate ADR"-Hinweis (Spec §7.1).

## 2. Warum kein Pflaster

DTLS ist sicherheits-kritisch. Eine halb-fertige oder un-auditierte DTLS-Record-
Layer ist gefährlicher als gar keine (false sense of security). [[feedback_bandaid_means_deeper_bug]]
+ [[feedback_no_mvp_build_product]] verbieten einen Stub. Die Entscheidung
hängt damit an der **Reife der verfügbaren pure-Rust-DTLS-Bausteine**, nicht am
ZeroDDS-Aufwand für das Wireup.

## 3. Kandidaten-Bewertung (Stand 2026-06)

| Kandidat | DTLS-Version | Reife | Eignung CoAP-Server | Urteil |
|---|---|---|---|---|
| **rustls** | — (nur TLS) | produktiv (auditiert) | ✗ kein Datagram-Mode | Basis für Krypto-Primitive, **kein DTLS** |
| **webrtc-dtls** (webrtc-rs / melekes) | 1.2 | produktiv, aber **WebRTC-scoped** | mittel — auf SRTP-Keying/ICE zugeschnitten, kein generischer Server-Loop; API-Schwergewicht | Möglich, aber Fremd-Scope + 1.2-only |
| **DusTLS** (ShadowJonathan) | 1.2 (PoC), 1.3 geplant | **WIP/PoC**, „nicht ecosystem-tuned" | ✗ noch nicht | Vielversprechend (re-nutzt rustls), aber **nicht audit-ready** |
| **HPTLS** (seceq) | TLS 1.3/1.2 + DTLS + QUIC | **neu**, „production-ready" beansprucht, OpenSSL-Interop | unklar | Beobachten — Reife-/Audit-Lage **unbelegt** 2026 |
| **OpenSSL/wolfSSL-Bindings** | 1.2/1.3 | produktiv (C) | gut | ✗ verletzt das pure-Rust + `forbid(unsafe_code)`-Prinzip des Workspace |

**Kernbefund:** Es gibt 2026 **keine audit-ready, ecosystem-Standard pure-Rust
DTLS-Server-Bibliothek**. webrtc-dtls ist am reifsten, aber WebRTC-scoped und
1.2-only; DusTLS ist die strategisch sauberste (rustls-basiert), aber noch PoC.

## 4. Mitigation, die heute schon greift: OSCORE

OSCORE (RFC 8613, ADR 0010) ist die **objekt-basierte** Alternative zu DTLS:
es schützt die CoAP-Nachricht selbst (nicht den Transport-Kanal). Vorteile
genau dort, wo DTLS schwach ist:

- **Proxy-tauglich** — Object-Security überlebt CoAP-Proxies; hop-by-hop-DTLS
  bricht an jedem Proxy.
- **Kein UDP-Record-Layer** — kein DTLS-Handshake/Cookie/Epoch nötig; baut nur
  auf HKDF + AEAD (im Workspace verfügbar).
- **Constrained-tauglich** — kleinerer Footprint als ein DTLS-Stack.

OSCORE deckt damit die **Vertraulichkeit + Integrität + Replay-Schutz** der
Anwendungsschicht ab, die sonst DTLS liefern würde — ohne den blockierten
DTLS-Record-Layer. Für reine PSK-/AEAD-Use-Cases ist OSCORE ein vollwertiger
Ersatz; DTLS bleibt nur für Szenarien nötig, die explizit Transport-Kanal-
Sicherheit + Zertifikats-Handshake fordern.

## 5. Empfehlung

> **Entscheidungs-Update (2026-06-12) — DTLS eingebaut.** Die ursprüngliche
> „DEFER"-Empfehlung (Punkt 2) ist **superseded**. Owner-Begründung
> (Spec-Completeness-Linie, [[feedback_spec_completeness_over_competition]]):
> „nicht audit-ready" allein ist kein Reject-Grund für ein **opt-in**-Profil —
> ZeroDDS selbst ist jung und bittet um Vertrauen, also darf es einem reifen-
> genug pure-Rust-DTLS dasselbe gewähren. **Umgesetzt mit `webrtc-dtls` (DTLS
> 1.2)**, NICHT hptls: hptls hat einen harten Rechts-Blocker (keine LICENSE-
> Grant-Dateien im Repo, `yourusername`-Platzhalter-URL), ist git-only und zieht
> einen eigenen unauditierten Krypto-Stack (`hpcrypt`, from-scratch RSA/Kurven/
> PQC) — formal nicht einbindbar. `webrtc-dtls` ist crates.io-publiziert,
> MIT/Apache mit echten Grant-Dateien, ~3,9 Mio Downloads, in webrtc-rs
> produktiv. Realisiert in `crates/coap-bridge` Feature **`dtls`**
> (`dtls_transport.rs`: `DtlsCoapServer`/`DtlsCoapClient`/`DtlsCoapSession`),
> e2e `dtls_coap_e2e.rs` (DTLS-Handshake + CoAP-GET→2.05-Content). **Opt-in +
> klar als experimentell gelabelt** (DTLS 1.2, nicht auditiert); der no_std-
> Codec-Core + Default-Build bleiben unberührt. Siehe **ADR 0011**.

1. **OSCORE fertigstellen** (ADR 0010) als primärer pure-Rust-Security-Pfad der
   CoAP-Bridge — deckt die meisten gesicherten Use-Cases ab, ohne DTLS.
2. ~~**DTLS-Record-Layer DEFER**~~ → **ERLEDIGT** via `webrtc-dtls` (Update
   oben). Re-evaluierung Richtung DTLS 1.3 / Audit bleibt eine Verbesserung,
   blockiert aber nichts mehr; die Config-Schicht (`dtls.rs`) + der neue
   `dtls_transport.rs` sind die Andock-Punkte.
3. **Kein OpenSSL/C-Binding** — verletzt das pure-Rust + `forbid(unsafe_code)`-
   Prinzip; würde die Audit-Aussagen des gesamten Security-Stacks verwässern
   ([[feedback_never_modify_vendor_binaries]] sinngemäß für den Trust-Boundary).
4. **Tracking:** dieser Reife-Gate wird in `docs/OPEN-ITEMS.md` + einem
   `coap-bridge`-Followup geführt; Kandidaten-Status quartalsweise prüfen.

## 6. Quellen

- rustls — <https://github.com/rustls/rustls> (TLS-only, kein DTLS)
- webrtc-dtls — <https://github.com/webrtc-rs/dtls> / <https://crates.io/crates/webrtc-dtls>
- DusTLS — <https://github.com/ShadowJonathan/dustls> (WIP DTLS 1.2-PoC, rustls-basiert)
- HPTLS — <https://github.com/seceq/hptls> (TLS/DTLS/QUIC, Reife unbelegt)
- RFC 7252 §9 (CoAP-Security-Modi), RFC 6347 (DTLS 1.2), RFC 9147 (DTLS 1.3)
- RFC 8613 (OSCORE) — die object-security-Mitigation, ADR 0010
