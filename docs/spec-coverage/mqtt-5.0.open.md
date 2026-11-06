# mqtt-5.0 — offene Punkte

Aggregat aus `mqtt-5.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §6 WebSocket-Transport für MQTT

**Status:** `partial` — die Client-Seite (`ws://`, `wss://` im Daemon) hält die vier Regeln und ist gegen Mosquitto getestet (die Tests sind `#[ignore]`, sie brauchen Mosquitto). Der Server (`crates/mqtt-bridge/src/server.rs`) nimmt keine WebSocket-Verbindung an; eigenes Item.

## §6 MQTT-Server über WebSocket

**Status:** `open` — `MqttBrokerServer` (`crates/mqtt-bridge/src/server.rs`) hat nur einen TCP-Listener.
