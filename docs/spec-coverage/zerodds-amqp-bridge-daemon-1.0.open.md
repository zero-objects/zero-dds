# zerodds-amqp-bridge-daemon-1.0 — offene Punkte

Aggregat aus `zerodds-amqp-bridge-daemon-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §6 DDS-QoS → AMQP-Behavior Map

**Status:** `partial` — Reliability, Durability (Header und Terminus), KEEP_LAST, Lifespan, Deadline-Annotation `x-opt-zerodds-deadline-missed`, Liveliness (Lease und Herzschlag) und Partition (Endpunkte, `dds:partition`, Abweisung) gebaut und auf dem Linux-Testhost grün. Die Lease ist an den DDS-Endpunkten geprüft: ein Writer mit 200 ms matcht den Reader eines 500-ms-Topics, einer mit 2000 ms nicht; ein Reader mit 1000 ms matcht den Writer, einer mit 300 ms nicht. Der Terminus-Default `durable` 1 gilt nur für das Target des Sender-Links. Der Herzschlag ging vorher erst beim nächsten 250-ms-Takt hinaus (gemessen 3 statt 5 in 1,6 s bei 600 ms idle-time-out); der Pump wacht jetzt zum fälligen Herzschlag auf. Offen: Backpressure für KEEP_ALL.

## §10 RTPS-Peer + RabbitMQ/Artemis/Qpid/Solace/ServiceBus

**Status:** `partial` — RabbitMQ 4.0.5, Artemis 2.44.0, Qpid Dispatch 1.19.0 und Solace PubSub+ Standard 10.26.0.9219 auf dem Linux-Testhost gemessen; ein Cyclone-Schreiber vor der Bridge gemessen (§12.3); der Solace-Job in CI nicht gelaufen (Runner-Konfiguration); Azure Service Bus nicht gemessen (Lizenzannahme für den Emulator).

## §10a Broker-Matrix Artemis/Qpid-Dispatch/Solace/Azure ServiceBus

**Status:** `partial` — Artemis, Qpid Dispatch und Solace PubSub+ Standard 10.26.0.9219 mit den fünf Fällen der Matrix gemessen (Solace: `<queue>` und `queue://<name>` in der Matrix, die Queues legt der Test über SEMP v2 an; abgelehnt wird der Empfänger einer Queue mit `no-access`, weil Solace jeden Sender annimmt; `topic://<name>` mit zwei Empfängern, von denen jeder jedes Sample erhält; die Bridge über TLS mit SASL PLAIN; Artemis: `<address>` und `queue://<name>` in der Matrix, `topic://<name>` mit zwei Empfängern, von denen jeder jedes Sample erhält; ein Neustart des Brokers; Dispatch: der Empfänger zuerst, das Ergebnis der Bridge über den Router zurück; Dispatch 1.19.0 gegen CPython 3.11 gebaut, weil es unter Python 3.13 mit SIGSEGV endet). CBS gebaut, gegen Azure nicht gemessen; eine Runde von `put-token`, die nicht binnen des Connect-Time-outs beantwortet ist (erste Runde und Erneuerung), beendet die Verbindung, der Back-off folgt. ---

## §11 Linux/macOS/Windows/Docker Layout

**Status:** `partial` — Linux, macOS und Docker wie oben; der Windows-Dienst reagiert nicht auf den Dienststeuerungs-Manager (kein Windows-Host, nicht gebaut).

## §12.3 Multi-Vendor

**Status:** `partial` — auf dem Linux-Testhost gemessen: ein Cyclone-Leser hinter der Bridge (pika und qpid-proton → RabbitMQ → Bridge → Cyclone, Instanzen 7 und 8 und der Dispose von 7) und ein Cyclone-Schreiber vor der Bridge (Cyclone → Bridge → RabbitMQ → qpid-proton: zwei Samples in Schreibreihenfolge mit dem Content-Type ihres Encapsulation-Headers, Cyclones Nutzlast, `durable` bei TRANSIENT_LOCAL, `dds:instance-handle` als KeyHash, `group-id` aus diesem KeyHash; der Dispose von 7 mit `dds:operation` `dispose`, dem KeyHash von 7 und dessen `group-id`). Der Lauf zeigte: Cyclone 11.0.1 sendet kein `PID_KEY_HASH`, und die Bridge gab `dds:instance-handle` als Nullen aus. Jetzt berechnet der Leser den KeyHash aus `type_file`. Ohne `type_file`, oder wenn Körper oder Schlüssel nicht als der Typ dekodieren, gehen ein solches Sample und ein Dispose oder Unregister eines solchen Schreibers mit dem Nil-Handle hinaus (Nullen, kein `group-id`); `key_hash_unavailable` wird einmal je Schreiber geloggt und in `errors_total` gezählt. Für den Marker meldet dcps das über `UserSample::Lifecycle::key_hash_unavailable`; vorher ging ein solcher Dispose mit den ersten 16 Oktetten des `serializedKey` als `dds:instance-handle` hinaus, ohne Logzeile. Gegen Cyclone ohne `type_file` gemessen: die beiden Samples und der Dispose von 7 mit 16 Nullen als `dds:instance-handle` und ohne `group-id`, `errors_total` 1; mit `type_file` 0. Die Logzeile selbst prüft der Unit-Test von dds_pump. Die Broker-Matrix ist ohne Azure Service Bus gemessen (§10a).
