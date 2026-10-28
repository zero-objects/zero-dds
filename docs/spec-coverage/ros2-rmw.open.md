# ros2-rmw — offene Punkte

Aggregat aus `ros2-rmw.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## `rmw_get_serialized_message_size`

**Status:** `partial` — der Walk ist implementiert und strukturell geprüft. Was fehlt, ist die Verhaltensprüfung „der zurückgegebene Wert ist tatsächlich ≥ der Länge, die `rmw_serialize` schreibt"; die braucht die rosidl-Introspection-Typesupports und damit einen ROS-2-Host. Rest als eigenes Item unten geführt.

## `rmw_get_serialized_message_size` — Schranken-Nachweis auf einem ROS-2-Host

**Status:** `open` — es gibt keinen Testfall, der für eine reale Nachricht `rmw_get_serialized_message_size` gegen die von `rmw_serialize` geschriebene Länge hält. Voraussetzung ist ein ROS-2-Host; der Fall gehört neben die vorhandenen rclcpp-e2e-Tests in `crates/rmw-zerodds-shim/rmw_c/`.

## RMW-Integration (`rmw_serialization_support_init`, `rmw_take_dynamic_message[_with_info]`)

**Status:** `partial` — Interface-Tabelle und RMW-Wiring laufen (`run_dynamic_typesupport_test.sh` gegen ROS 2 Jazzy; zuletzt gemessen 17/17 grün, vor den oben genannten Checks). Zwei benannte, bewusste Lücken, keine davon verdeckt: (1) `dynamic_data_serialize` kennt keine verhandelte `DataRepresentation`-QoS — die Schnittstelle gibt der Funktion keinen Writer/QoS-Kontext — und schreibt deshalb immer `@final`, Little-Endian, XCDR1 (PLAIN_CDR1), identisch zur Framing-Wahl des statischen Introspection-Codecs in `rmw_zerodds.c`. (2) `long double`/`float128` wird nur für die x86-64-80-Bit-Extended- und die aarch64-binäre128-Speicherform gebrückt (`Float128:: {to,from}_x87_padded_bytes`/`{to,from}_le_bytes`); 32-Bit-ARM und PowerPCs Double-Double-`long double` werden mit RCUTILS_RET_UNSUPPORTED abgelehnt statt geraten.
