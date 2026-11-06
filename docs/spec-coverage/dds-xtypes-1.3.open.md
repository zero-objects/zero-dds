# dds-xtypes-1.3 — offene Punkte

Aggregat aus `dds-xtypes-1.3.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §7.5.1.2.3.1 C: zusätzliche Kopierfunktion mit wählbarem Verhalten

**Status:** `partial` — die C-Bindung erzeugt `zd_copy_with_<T>` mit Tiefe (`ZD_COPY_DEEP`, `ZD_COPY_SHALLOW`) und Aktion (`ZD_COPY_REUSE`, `ZD_COPY_REPLACE`, `ZD_COPY_RELEASE`); der Test prüft nur, dass die Konstanten und die Vorgabe (tief, wiederverwenden) im erzeugten Text stehen. Kein Test übersetzt den erzeugten C-Code und prüft das Verhalten je Kombination.

## §7.5.1.2.3.1 Table 45 — Shallow Copy, Replace

**Status:** `partial` — der Zweig steht in `zd_copy_with_<T>`; kein Test führt den erzeugten C-Code aus und prüft dieses Verhalten.

## §7.5.1.2.3.1 Table 45 — Shallow Copy, Release

**Status:** `partial` — der Zweig steht in `zd_copy_with_<T>`; kein Test führt den erzeugten C-Code aus und prüft dieses Verhalten.

## §7.5.1.2.3.1 Table 45 — Deep Copy, Reuse

**Status:** `partial` — der Zweig steht in `zd_copy_with_<T>`; kein Test führt den erzeugten C-Code aus und prüft dieses Verhalten.

## §7.5.1.2.3.1 Table 45 — Deep Copy, Replace

**Status:** `partial` — der Zweig steht in `zd_copy_with_<T>`; kein Test führt den erzeugten C-Code aus und prüft dieses Verhalten.

## §7.5.1.2.3.1 Table 45 — Deep Copy, Release

**Status:** `partial` — der Zweig steht in `zd_copy_with_<T>`; kein Test führt den erzeugten C-Code aus und prüft dieses Verhalten.
