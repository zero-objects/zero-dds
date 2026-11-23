# zerodds-tcp-transport-1.0 — offene Punkte

Aggregat aus `zerodds-tcp-transport-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §1.1 Locator-Kind TCPv4 / TCPv6

**Status:** `partial` — gebaut: die Art ist Eigenschaft des Locators, der prozessweite Schalter `set_tcp_kinds_fast_dds` ist entfernt; `TcpTransport::with_locator_kinds` und `transport.tcp_locator_kinds` wählen 4 und 8 je Transport und je Runtime; Lesen behält die geschriebene Art; ein Ziel mit 4 oder 8 wird über den TCP-Transport gesendet. Mit dem Dialekt `cyclone` (§4.1) kündigt eine Runtime 4 und 8 an und einen Listener-Port für Metatraffic und Nutzdaten, wie Cyclone DDS mit `TCP/Port` (`crates/dcps/tests/runtime_config_layering.rs::tcp_4_1_the_cyclone_dialect_is_a_setting`; mit einem laufenden Cyclone DDS: `crates/dcps/tests/cyclone_live_tcp.rs::tcp_4_1_a_zerodds_writer_reaches_a_cyclone_subscriber_without_a_listener`). Die zitierten Tests in `zerodds-rtps`, `zerodds-transport-tcp` und `zerodds-dcps` sind auf dem Linux-Testhost grün. Mit dem Dialekt `fastdds` (§6.1) kündigt eine Runtime 4 und 8 mit dem logischen Port in den oberen 16 Bit des Ports an, und ein Locator der Art 4 oder 8 mit einem Port über 65535 wird als einer von Fast DDS gelesen und erreicht (`crates/transport-tcp/src/tcp_transport.rs::tests::tcp_6_1_a_locator_with_a_logical_port_is_a_fast_dds_destination`, `crates/dcps/tests/fastdds_live_tcp.rs`). Rest: Arten und Port-Layout des Dialekts von RTI Connext (§6.2) fehlen; `transport.tcp_locator_kinds: fast-dds` allein kündigt 4 und 8 ohne logischen Port an, einen Locator, den Fast DDS nicht adressiert (es braucht den Dialekt `fastdds`).

## §6.2 RTI-TLS-orientierter Handshake

**Status:** `open` — keine Betriebsart mit dem Protokoll von RTI Connext. Es fehlen: das TCP-Protokoll von RTI (eigene Kontrollnachrichten, Kontroll- und Datenverbindungen getrennt, die Klassen-IDs `tcpv4_lan`/`tcpv4_wan`/`tlsv4_lan`/`tlsv4_wan` in der Locator-Art), die TLS-Schicht darunter und ein Test gegen RTI Connext. Der Text oben ist nicht an RTI geprüft; RTI veröffentlicht keine Spezifikation des Protokolls. Auf dem Linux-Testhost dieser Einheit ist kein RTI Connext installiert, gegen das eine Umsetzung geprüft werden könnte.
