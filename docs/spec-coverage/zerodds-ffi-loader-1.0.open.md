# zerodds-ffi-loader-1.0 — offene Punkte

Aggregat aus `zerodds-ffi-loader-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §2.5 Thread-Safety

**Status:** `open` — kein Test ruft die ABI aus mehreren Threads. `Send`/`Sync` stehen im Typsystem, gemessen ist die Nebenläufigkeit nicht.

## §6 Library + Header + Per-Sprach-Packages

**Status:** `partial` — Library, Header und pkg-config sind für Linux und macOS in den Paketen und werden gegen die Klausel geprüft; die PyPI-Distribution heisst wie die Klausel sie nennt. Nicht erfüllt sind der Windows-Anteil und drei der sechs Per-Sprach-Packages; beides als §6b geführt.

## §6b Windows-Header/-Library und die fehlenden Per-Sprach-Packages

**Status:** `open` — `packaging/windows/msi/zerodds.wxs` hat das `Devel`-Feature ausdrücklich entfernt, also liefert kein Windows-Paket Header, DLL oder Import-Library aus. Von den Per-Sprach-Packages existiert im Baum nur das PyPI-Wheel unter dem genannten Namen; das npm-Paket heisst `@zerodds/wire` statt `@zerodds/sdk`/`@zerodds/wasm`, ein NuGet- und ein pub.dev-Package gibt es nicht, und das Maven-Modul trägt die Koordinaten `org.zerodds:omgdds` statt `eu.ifyna:zerodds-java-omgdds`.
