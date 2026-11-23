# amqp-1.0 — offene Punkte

Aggregat aus `amqp-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §0.2 Conformance

**Status:** `partial` — gebaut sind die Protokoll-Aushandlung (§2.2), Part 1, Part 2 auf der antwortenden Seite und als Client, Part 3, Part 4 und Part 5 mit SASL und TLS von Beginn der Verbindung an; die Items der Parts nennen den Stand je Abschnitt. Es fehlen: was die Items §2.5.1 bis §2.5.3, §2.6.1, §2.6.3, §2.6.7 bis §2.6.10, §2.6.13, §2.8.14, §3.3, §3.4.1, §3.4.5, §3.5.1/§3.5.2, §3.5.3, §3.5.5/§3.5.6, §3.5.9 bis §3.5.13, §5.2 als fehlend nennen; SCRAM-SHA-1 (§0.2: "SHOULD support the PLAIN [RFC4616] and SCRAM-SHA1 [RFC5802] mechanisms"; gebaut sind PLAIN und SCRAM-SHA-256). Part 4 (Transactions) hat eigene Items, §4.1 bis §4.5.8.

## §2.5.1 bis §2.5.3 Establishing and Ending a Session

**Status:** `partial` — jede Seite kann nur eine Rolle: `Peer` beginnt selbst keine Session und schließt bei einem `begin` mit `remote-channel` mit `amqp:not-allowed`; der Client (`broker.rs`) beantwortet ein `begin`, das der Broker von sich aus sendet, nicht, sondern meldet einen Protokollfehler.

## §2.6.1, §2.6.2 Naming a Link, Link Handles

**Status:** `partial` — `stolen` gilt innerhalb einer Verbindung; ein Link gleichen Namens, den derselbe Container über eine zweite Verbindung anhängt, lässt den ersten bestehen (die Verbindungen eines Listeners teilen kein Link-Verzeichnis).

## §2.6.3 Establishing or Resuming a Link

**Status:** `partial` — Anlegen ist gebaut. Wiederaufnahme: der Listener hält keinen Terminus über das Ende eines Links hinaus (er meldet `expiry-policy` `link-detach`), seine `unsettled`-Map ist deshalb immer leer; der Client sendet `attach.unsettled` nie und kann einen Link nicht wiederaufnehmen.

## §2.6.7 bis §2.6.10 Flow Control, Synchronous Get, Asynchronous Notification, Stopping a Link

**Status:** `partial` — die antwortende Seite ist auf beiden Seiten gebaut: der Listener (`Peer`), die Links des Daemons (Topic-Links und `$cbs`) und die Links des Transaktions-Controllers leeren auf `drain` und beantworten `echo`, mit und ohne Handle. Offen: kein Client setzt `flow.echo`, das Anhalten mit Bestätigung (§2.6.10) ist auf der Client-Seite nicht möglich; `flow.drain` setzt nur `TxnClient::receive` (Abruf mit Zeitgrenze, §2.6.8), die empfangenden Links des Daemons fragen nie danach. Gegen einen Broker, der selbst `drain` oder `echo` sendet, ist das Verhalten nur mit dem geskripteten Broker gemessen, nicht gegen einen Fremd-Broker.

## §2.6.13 Resuming Deliveries

**Status:** `partial` — keine Seite hält unerledigte Zustellungen über einen Link hinaus: der Listener gibt sie beim Ende des Links an den Knoten zurück (Default-Outcome `released`), der Client sendet weder `attach.unsettled` noch `transfer.resume`. Der Abgleich zweier nicht leerer Maps und `incomplete-unsettled` sind nicht gebaut.

## §2.8.14 Error

**Status:** `partial` — `info` wird nicht gelesen und nicht geschrieben; `AmqpError` hat kein Feld dafür.

## §3.3 Distribution Nodes

**Status:** `partial` — gebaut sind AVAILABLE und ACQUIRED an einer Warteschlange, die jede Nachricht an einen Link abgibt (`move`). Der Zustand ARCHIVED fehlt: es gibt keine Knotenart, die eine Nachricht nach der Abgabe behält (`copy`, §3.5.2).

## §3.4.1, §3.4.6 Received, Resuming Deliveries Using Delivery States

**Status:** `partial` — nur der Codec. Kein Endpunkt sendet `received`, und keiner setzt eine Übertragung an einer gemeldeten Stelle fort (siehe §2.6.13).

## §3.4.2 bis §3.4.5 Accepted, Rejected, Released, Modified

**Status:** `partial` — `modified.message-annotations` wird nicht gelesen und nicht geschrieben (`Outcome::Modified` hat kein Feld dafür); alles andere ist gebaut.

## §3.5.1, §3.5.2, §3.5.7, §3.5.8 Filtering, Distribution Modes, Filter Set

**Status:** `partial` — gebaut sind der Codec und `move` ohne Filter, so gemeldet. Es fehlen: die Verteilungsart `copy` (eine Knotenart, die Nachrichten nach der Abgabe behält, mit dem Zustand ARCHIVED aus §3.3) und jede Auswertung eines Filters; ein angeforderter Filter und `copy` werden nicht gewährt (der Test `amqp10_2_6_12_…` fordert beides an und prüft, dass `move` und kein Filter gemeldet werden).

## §3.5.3, §3.5.4 Source, Target

**Status:** `partial` — der Client (`broker.rs::LinkSpec`) sendet von Quelle und Ziel nur `address` und `durable`; `expiry-policy`, `timeout`, `dynamic`, `distribution-mode`, `filter`, `default-outcome`, `outcomes` und `capabilities` kann er nicht anfordern und liest sie aus der Antwort des Brokers nicht.

## §3.5.5, §3.5.6 Terminus Durability, Terminus Expiry Policy

**Status:** `partial` — gebaut sind der Codec und der nicht dauerhafte Terminus, der mit dem Link endet (`durable` `none`, `expiry-policy` `link-detach`, `timeout` 0, so gemeldet). Es fehlen am Listener: `durable` `configuration` und `unsettled-state`, die Ablaufregeln `session-end`, `connection-close` und `never` und `timeout`; dafür müsste ein Terminus das Ende seines Links überdauern, was auch die Wiederaufnahme aus §2.6.3 und §2.6.13 trägt. Der Client kann nur `durable` anfordern (§3.5.3).

## §3.5.9 bis §3.5.13 Node Properties, Lifetime Policies

**Status:** `partial` — nur `delete-on-close` ist gebaut; wer eine der drei anderen Lebensdauern wünscht, bekommt `delete-on-close` gemeldet. `supported-dist-modes` wird nicht gemeldet.

## §5.2 TLS

**Status:** `open` — der Header mit Protokoll-Id 2 ist auf keiner Seite gebaut: der Listener beantwortet ihn wie jede andere nicht unterstützte Protokoll-Id mit seinem Header und schließt (§2.2), der Client sendet ihn nie. TLS gibt es nur von Beginn der Verbindung an (§5.2.1).
