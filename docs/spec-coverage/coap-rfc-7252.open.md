# coap-rfc-7252 — offene Punkte

Aggregat aus `coap-rfc-7252.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §5.7 Proxying

**Status:** `partial` — Proxy-Uri-Option, Forward-/Reverse-Modus und Hop-Grenze sind als Konfiguration gebaut; das Weiterleiten eines Requests fehlt (§5.7a).

## §5.7a Weiterleiten durch einen Proxy

**Status:** `open` — `crates/coap-bridge/src/caching_proxy.rs` hält nur `ProxyConfig`/`ProxyMode`; kein Pfad sendet einen Request an das Ziel weiter.

## §7.1-§7.2 Service + Resource Discovery

**Status:** `partial` — Resource-Discovery über `/.well-known/core` und die Multicast-Adressen sind gebaut und geprüft; Service-Discovery per Multicast braucht den Multicast-Empfang aus §8a.

## §8 Multicast Operation

**Status:** `partial` — Multicast-Adressen, NON-only-Prüfung, Token-Pflicht und Leisure-Zeit sind gebaut und geprüft; Gruppenbeitritt, Senden und Empfangen im Daemon fehlen (§8a).

## §8a Multicast-Versand und -Empfang im Daemon

**Status:** `open` — `crates/coap-bridge/src/` tritt keiner Multicast-Gruppe bei (kein `join_multicast_v4`/`join_multicast_v6`); die Helfer aus `multicast.rs` werden von keinem Socket-Pfad benutzt.

## §9 DTLS Mode

**Status:** `partial` — NoSec, PreSharedKey und Certificate laufen über den DTLS-Transport; RawPublicKey ist nur als Konfiguration vorhanden (§9a).

## §9a RawPublicKey-Modus im DTLS-Transport

**Status:** `open` — `crates/coap-bridge/src/dtls.rs` kennt `DtlsMode::RawPublicKey` als Konfiguration; `crates/coap-bridge/src/dtls_transport.rs` handelt nur PSK und Zertifikat aus (`DtlsPeerIdentity::{Psk, Certificate}`), ein Handshake mit Raw Public Key (RFC 7250) fehlt.

## §11.1 Parsing und URI-Verarbeitung

**Status:** `partial` — Robustheit des Decoders und die URI-Zerlegung sind getestet; kein Test prüft, dass Zugriffsentscheidung (Topic-ACL) und Auslieferung dieselbe Zerlegung der URI verwenden.

## §11.2 Cache: keine Auslieferung an Anfragen mit geringerer Transportsicherheit

**Status:** `open` — `CoapCache` (`crates/coap-bridge/src/caching_proxy.rs`) speichert und liefert Einträge ohne Bezug auf die Transportsicherheit der Anfrage.

## §11.2 coaps-Antworten nicht im geteilten Cache

**Status:** `open` — `CoapCache` unterscheidet nicht zwischen Antworten auf `coap`- und `coaps`-Anfragen.

## §11.3 Keine große Verstärkung ohne Authentifizierung

**Status:** `open` — kein Code gefunden, der die Antwortgröße für nicht authentifizierte Anfragen begrenzt.

## §11.3 Multicast-Anfragen nur authentifiziert und nur für bestimmte Ressourcen

**Status:** `open` — der Daemon (`crates/coap-bridge/src/daemon/`) unterscheidet Multicast- und Unicast-Anfragen nicht und schränkt Multicast weder auf authentifizierte Anfragen noch auf Ressourcen ein.

## §11.3 Multicast-Ziel über IPV6_RECVPKTINFO erkennen

**Status:** `open` — kein Code in `crates/coap-bridge` liest die Zieladresse eines empfangenen Pakets.

## §11.6 Constrained-Node Considerations

**Status:** `open` — die OSCORE-Kryptografie verwendet `aes`, `ccm`, `hmac` und `sha2` aus RustCrypto; kein Test prüft die Laufzeitkonstanz. Die Bridge erzeugt keine Schlüssel.

## §12.3 CoAP Content-Formats Registry

**Status:** `open` — `crates/coap-bridge/src/daemon/server.rs` setzt 40 für `application/link-format`, ohne Test; für die übrigen Einträge gibt es keine Konstanten.
