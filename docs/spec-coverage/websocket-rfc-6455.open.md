# websocket-rfc-6455 — offene Punkte

Aggregat aus `websocket-rfc-6455.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §10.2 Origin prüfen

**Status:** `open` — der Handshake (`crates/websocket-bridge/src/handshake.rs`) und der Daemon prüfen das Feld `Origin` nicht.

## §10.3 Neuer, nicht vorhersagbarer Masking-Key je Frame

**Status:** `partial` — je Frame entsteht ein neuer Schlüssel; der Test prüft, dass aufeinanderfolgende Schlüssel verschieden sind. Kein Test prüft die Quelle der Zufallszahlen; das Modul bietet mit `ws_5_3_insecure_splitmix_provider_implements_trait` auch einen vorhersagbaren Provider an.

## §10.4 Grenzen für Frame- und Nachrichtengröße

**Status:** `partial` — die Gesamtgröße nach dem Zusammensetzen ist begrenzt und getestet; kein Test prüft eine Grenze für die Größe eines einzelnen Frames (z. B. eine angekündigte Länge von 2^60).

## §10.7 Ungültige Daten

**Status:** `partial` — ein Masking-Verstoß schließt mit 1002, eine ungültige Anmeldung im Handshake bekommt HTTP 401. Nicht einzeln geprüft: der Close-Code für jede andere Art ungültiger Daten nach dem Handshake.
