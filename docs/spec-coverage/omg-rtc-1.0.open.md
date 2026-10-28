# omg-rtc-1.0 — offene Punkte

Aggregat aus `omg-rtc-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §1.3 Platform-Specific Models

**Status:** `partial` — Local PSM (§6.3) umgesetzt und belegt; Lightweight CCM PSM (§6.4) und CORBA PSM (§6.5) fehlen und werden als eigene `open`-Items geführt.

## §5.2.2.2 LightweightRTObject Interface

**Status:** `partial` — sechs der zehn Annex-A-Operations sind umgesetzt und belegt. `exit`, `get_context`, `get_owned_contexts` und `get_context_handle` fehlen; sie setzen ein Owner-Context-Register im `LightweightRtObject` voraus, das der Typ nicht führt. Rest als eigenes `open`-Item „§5.2.2.2 Owner-Context-Operations".

## §5.2.2.2 Owner-Context-Operations (`exit` / `get_context` / `get_owned_contexts` / `get_context_handle`)

**Status:** `open` — `LightweightRtObject` führt kein Register der Contexts, die es besitzt (nur der, an denen es teilnimmt), und hält keine Referenz auf den Context-Objekt-Typ. Die vier Operations setzen beides voraus. Rest-Item zu §5.2.2.2.

## §6.4 Lightweight CCM PSM

**Status:** `open` — es gibt keinen RTC→LwCCM-Adapter im Baum. `crates/ccm/src/lightweight.rs` filtert ein CCM-IDL-Modell auf das LwCCM-Profil und kennt kein RTC; `crates/rtc/` kennt kein CCM. Der Crate-Doc-Header von `crates/rtc/src/lib.rs` führt §6.4 selbst unter „What is not covered". Die Spec erlaubt Compliance über einen einzigen PSM (§2); umgesetzt ist der Local PSM (§6.3).

## §6.5 CORBA PSM

**Status:** `open` — es gibt keinen RTC→CORBA-Adapter und keine RTC-Annex-A-IDL, die durch die Codegen-Crates liefe. Der Crate-Doc- Header von `crates/rtc/src/lib.rs` führt §6.5 selbst unter „What is not covered". Die Spec erlaubt Compliance über einen einzigen PSM (§2); umgesetzt ist der Local PSM (§6.3).

## Annex A — Vollständige RTC-IDL-Datei

**Status:** `partial` — die Annex-A-Fläche ist bis auf `exit`, `get_context`, `get_owned_contexts` und `get_context_handle` ausgeführt; der Rest ist das `open`-Item zu §5.2.2.2.
