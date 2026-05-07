# `.zddsrec` v1.0 — Recording-Format-Spec

ZeroDDS Vendor-Spec. In `crates/recorder` (`zerodds-recorder`) implementiert.

## Ziele

* **Kompakt**: keine textuellen Annotationen im Hot-Path. CDR-Payloads
  bleiben unverändert; Header sammelt Metadata einmalig.
* **Streamfreundlich**: Frames sind eigenständig parsebar nach
  einem einmal gelesenen Header. Reader und Writer sind beide
  inkrementell.
* **Spezifizierbar**: byte-genau in 1 Seite, keine Reflection-Magie.

## Datei-Struktur

Endianness: **little-endian** für alle Multi-Byte-Integer.

```
+========== Header ==========+
| Magic    "ZDDS"  (4 byte)  |
| Version  u32     (LE)      |  // = 1 fuer diese Spec
| TimeBase i64     (LE) ns   |  // UNIX-Epoch-Anchor
| ParticipantCount u32 (LE)  |
| TopicCount       u32 (LE)  |
| Participants[]             |
|   Guid       16 bytes      |
|   NameLen    u32 (LE)      |
|   Name       UTF-8         |
| Topics[]                   |
|   TypeLen    u32 (LE)      |
|   TypeName   UTF-8         |
|   NameLen    u32 (LE)      |
|   Name       UTF-8         |
+========== Frames ==========+
| FrameMagic   'F'  (1 byte) |
| TimestampDelta i64 (LE) ns |
| ParticipantIdx u32 (LE)    |
| TopicIdx       u32 (LE)    |
| SampleKind     u8          |  // 0=Alive 1=Disposed 2=Unregistered
| PayloadLen     u32 (LE)    |
| Payload        bytes       |
+============================+
| FrameMagic 'F' ... weitere |
+============================+
```

EOF: kein expliziter Marker — der Reader stoppt wenn der Cursor das
File-Ende erreicht.

## Indizes

`ParticipantIdx` und `TopicIdx` sind 0-basierte Indizes in die
entsprechenden Header-Listen. Wenn ein neuer Participant oder Topic
mitten im Stream auftaucht, kann er **nicht** nachträglich
appendet werden — der Recorder muss vor dem ersten Frame alle
möglichen Sources kennen. Für Live-Discovery gilt: einen
Header pro abgeschlossenem Bereich, mehrere `.zddsrec`-Files pro
Session.

(Reserve-Hook fuer eine zukuenftige Major-Version: `IndexAddFrame` mit eigenem Magic-Byte als
optionale Inline-Erweiterung.)

## SampleKind-Codierung

| Wert | Spec-Mapping (DDS 1.4 §2.2.4.4.5) |
|------|-----------------------------------|
| 0    | `ALIVE`                           |
| 1    | `NOT_ALIVE_DISPOSED`              |
| 2    | `NOT_ALIVE_UNREGISTERED`          |

Andere Werte: Reader returnt `BadSampleKind(b)`-Fehler.

## Versionierung

`Version = ZDDSREC_VERSION` (= 1). Reader lehnt
`UnsupportedVersion(v)` für `v > ZDDSREC_VERSION` ab — ältere
Versionen können Reader-seitig im Compatibility-Pfad gelesen
werden.

Backward-incompatible Aenderungen heben die Major-Version. Additive
Aenderungen, die mit Default-Werten read-only-rückwaerts-kompatibel
bleiben (z.B. neue Frame-Magics ignorieren), bleiben in derselben
Major-Version.

## Use-Cases

* **Crash-Reproduce**: Production-Recorder schreibt 24h-Trace,
  Test-Setup spielt mit `--time-scale 60` (1h Wallclock) ab.
* **Forensic Diff**: zwei Traces mit identischem `TimeBase` zeitlich
  überlagern.
* **Soak-Replay**: Chaos-Test-Suite (F.2) injiziert auf einem Replay
  realistische Loss-/Jitter-Pattern.

## Tooling

| Tool                                      | Funktion             |
|-------------------------------------------|----------------------|
| `crates/recorder` library                 | API fuer Apps        |
| `tools/replay` Bin (`zerodds-replay`)     | inspect/dump/replay  |
| `tools/recorder-bridge`                   | Live-record-from-domain |

## Stabilitaet und Roadmap

`zerodds-recorder` 1.0 deckt das core-Format + Reader + Writer +
in-Process-`RecordingSession`. Die folgenden Erweiterungen sind als
**additive** Major-2.0-Hooks vorgesehen, ohne dass die 1.0-Wire-Form
zu brechen:

* Live-Recording aus DcpsRuntime via `zerodds-c-api`-Hook (Pfad ueber
  `tools/recorder-bridge`).
* Live-Replay → `zerodds_writer_write` Re-Injection.
* `IndexAddFrame` fuer inline-discovery additions (Reserve-Magic-Byte
  bereits am End-Of-Stream-Cursor anschliessbar).
* Optionale `zlib`/`zstd` Frame-Compression mit separatem FrameMagic
  als opt-in Erweiterung.

Der Reader 1.0 lehnt unbekannte FrameMagic-Bytes mit
`ReadError::UnknownFrameMagic` ab — additive Frame-Typen koennen
sicher hinzugefuegt werden.
