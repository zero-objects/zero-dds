# OASIS AMQP 1.0 — Open + Partial Items

Aggregat aus `amqp-1.0.md`. Nicht von Hand pflegen — vor jedem Audit-
Lauf löschen und aus dem Hauptfile neu generieren.

## Open

— keine.

## Partial

### §1.1 Type System (Primitive/Described/Composite/Restricted)

**Status:** `partial` — Primitive-Types vollständig abgedeckt
(siehe §1.6.x); Described-/Composite-/Restricted-Types werden vom
Caller-Layer (Performatives in §2.7, Message in §3) als
described-composite über Primitive-Types serialisiert. Eine
Allgemein-Codec-Schicht für beliebige Composites ist nicht
implementiert. Aufwand 0.5-1 PW falls als Library-API verlangt.

## Decision-Records (`n/a (rejected)`)

### §5 SASL — `n/a (rejected)`

**Begründung:** Die SASL-Mechanism-Implementation (PLAIN, ANONYMOUS,
SCRAM-SHA-256, EXTERNAL etc.) ist eine Crypto-/Auth-Schicht außerhalb
des Wire-Formats. ZeroDDS-AMQP-Bridge transportiert SASL-Frames im
Frame-Format (§2.3.1.4 FrameType=0x01) korrekt, aber wählt keine
Mechanism-Implementation aus. Caller (Anwendung oder vorgelagertes
Auth-Modul) entscheidet pro Verbindung über die SASL-Implementation;
das passt zum Architektur-Prinzip "Security ist eigenständige Schicht"
(siehe `zerodds-security-1.2.md`).

**Impl-Auswirkung:** Eine eigene SASL-Implementation würde mindestens
ein neues Crate `crates/amqp-sasl/` einführen, mit Abhängigkeit zu
`crates/security-crypto` (für SCRAM-Hash-PBKDF2/HMAC) sowie
TLS-Reuse über `crates/security-pki` (für SASL-EXTERNAL). Aufwand
2-3 PW pro Mechanism. Wartung wegen IANA-SASL-Registry-Drift
(neue Mechanismen, deprecated Mechanismen) wäre laufend.

**Impl-Vorteil:** Drop-in-AMQP-Sessions ohne Caller-seitige
SASL-Bibliothek; nützlich falls ZeroDDS-AMQP-Bridge in eingebetteten
Targets ohne externe Auth-Stacks integriert werden soll. Cross-Vendor-
Interop mit RabbitMQ/ActiveMQ ohne Caller-Logik. Aktueller Bedarf
nicht ausgewiesen — Reaktivierung wenn ein Bestand-Migrationsszenario
ohne Caller-Auth-Layer auftritt.
