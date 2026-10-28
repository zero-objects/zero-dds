# zerodds-uds-transport-1.0 — offene Punkte

Aggregat aus `zerodds-uds-transport-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §5.1 Volume-Mount als Permission-Grenze

**Status:** `partial` — die Prozessgrenze ist belegt, die Container-Grenze nicht: es gibt keinen Test, der zwei Container mit gemountetem Volume startet und die UID-Mapping- und SELinux-Fragen tatsächlich durchspielt. Der fehlende Nachweis ist als §5.2 offen geführt.

## §5.2 Nachweis über eine echte Container-Grenze

**Status:** `open` — ohne Beschreibung im Hauptfile.

## §6.2 Windows

**Status:** `open` — auf Windows übersetzt der Crate zu einer leeren Bibliothek (`crates/transport-uds/src/lib.rs`: `#![cfg(unix)]`). Windows bietet seit Windows 10 (Build 17063) Sockets der Familie `AF_UNIX` an (Angabe von Microsoft, hier nicht geprüft); der Transport nutzt sie nicht.
