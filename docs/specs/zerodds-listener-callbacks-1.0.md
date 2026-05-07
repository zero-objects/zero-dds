# `zerodds-listener-callbacks` v1.0 — Vendor-Spec

ZeroDDS Vendor-Spec. In `crates/zerodds-c-api/src/listener_ffi.rs`
implementiert; C-Struktur-Definitionen via cbindgen in
`crates/zerodds-c-api/include/zerodds.h` exportiert.

**Status:** Draft 2026-05-06.

## Motivation

Die DDS-Spec (DDS 1.4 §2.2.4 *Listeners, Conditions, and Wait-sets*)
definiert das Listener-Konzept normativ nur fuer **Sprach-PSM**, die
Klassen unterstuetzen (DDS-PSM-Cxx 1.0 §7.5.9, DDS-Java-PSM 1.0 §8.7,
DDS-PSM-CSharp formell nicht standardisiert). Fuer **C** existiert
kein normativer Listener-Mapping-Path — RTI Connext, Eclipse Cyclone
und Fast-DDS haben jeweils eigene proprietaere C-Listener-APIs ohne
Cross-Vendor-Kompatibilitaet.

ZeroDDS spezifiziert hier eine **vollstaendige spec-konforme C-FFI-
Listener-API** als Vendor-Extension. Die API ist:

- **Cross-Language-Hub:** wird von `crates/cpp/` (DataWriterListener),
  `crates/cs/` (DataWriterListener interface) und `crates/ts-node/`
  (callbacks) konsumiert.
- **NativeAOT-kompatibel:** keine Reflection, keine GC-Allokationen
  im Callback-Pfad.
- **Strict-Pol Phase-1**: Alle 17 Spec-Listener-Callbacks (DDS 1.4
  §2.2.4 Tab. 2.10) sind exponiert. Aktivierung der einzelnen
  Callbacks (Wireup an Runtime-Worker-Thread) erfolgt schrittweise —
  siehe §6 Status-Phasen.

## Ziele

- **Spec-Vollstaendigkeit:** Jeder normative Listener-Callback aus DDS
  1.4 §2.2.4 hat einen C-FFI-Funktions-Pointer-Slot.
- **Bubble-Up-Pfad:** Aggregator-Listener (Publisher → DataWriter,
  Subscriber → DataReader, DomainParticipant → alle) per Spec
  §2.2.4.2 unterstuetzt — Status-Bits werden hierarchisch propagiert.
- **Status-Mask-Filter:** Caller waehlt welche Callbacks aktiv sind
  (Spec §2.2.4.2.1.4).
- **Thread-Safety-Vertrag** explizit dokumentiert.

## Nicht-Ziele

- Generic-Listener-Container (`AnyXxx`-Typen): bleiben den Sprach-
  Bindings ueberlassen.
- Synchrone Listener-Calls aus `*_set_listener` heraus (Spec verbietet
  das ohnehin per "Listener calls happen in implementation-specific
  threads").
- Listener-Chains (mehrere Listener pro Entity): Spec-konform 1:1.

## §1 Architektur

### §1.1 Funktions-Pointer-Tabelle (vtable)

Jeder Entity-Typ hat eine eigene `Zerodds*Listener`-Struktur als
`#[repr(C)]` mit Funktions-Pointern. Alle Pointer sind optional
(NULL = Callback ignoriert).

```c
typedef struct {
    void* user_data;                                        // §1.2

    void (*on_liveliness_lost)(void* user_data, zerodds_DataWriter* dw);
    void (*on_offered_deadline_missed)(void* user_data, zerodds_DataWriter* dw);
    void (*on_offered_incompatible_qos)(void* user_data, zerodds_DataWriter* dw);
    void (*on_publication_matched)(void* user_data, zerodds_DataWriter* dw);
} zerodds_DataWriterListener;
```

### §1.2 `user_data`-Slot

Pro Listener-Struktur ein `void* user_data`-Feld. Wird unveraendert
an jeden Callback gereicht. Caller verpackt darin sein State-Object
(z.B. `JNIEnv*` + `jobject`-Pair fuer Java-JNI, `GCHandle` fuer C#,
`PyObject*` fuer Python). Lifetime ist Caller-Pflicht.

### §1.3 Set/Get-API

```c
int zerodds_dp_set_listener(zerodds_DomainParticipant* p,
                             const zerodds_DomainParticipantListener* l,
                             uint32_t status_mask);
const zerodds_DomainParticipantListener* zerodds_dp_get_listener(
    zerodds_DomainParticipant* p);
```

Pro Entity-Typ ein eigenes Paar. NULL-Pointer bei `set_*` clears.

## §2 Listener-Inventar (alle 6 Entity-Typen)

### §2.1 DomainParticipantListener (DDS 1.4 §2.2.4.2.1)

```c
typedef struct {
    void* user_data;
    void (*on_inconsistent_topic)(void* user_data, zerodds_Topic* t);
    void (*on_data_on_readers)(void* user_data, zerodds_Subscriber* sub);
} zerodds_DomainParticipantListener;
```

DomainParticipantListener ist Aggregator fuer ALLE Children-Status-
Events. RC1 hat den Aggregator-Pfad fuer 2 Bubble-Up-Bits exponiert
(`INCONSISTENT_TOPIC` aus Topic-Children, `DATA_ON_READERS` aus
Subscriber-Children). Vollausbau auf alle 13 Bits ist Phase-2.

### §2.2 PublisherListener (DDS 1.4 §2.2.4.2.2)

```c
typedef struct {
    void* user_data;
    void (*on_offered_deadline_missed)(void*, zerodds_DataWriter*);
    void (*on_liveliness_lost)(void*, zerodds_DataWriter*);
    void (*on_offered_incompatible_qos)(void*, zerodds_DataWriter*);
    void (*on_publication_matched)(void*, zerodds_DataWriter*);
} zerodds_PublisherListener;
```

### §2.3 SubscriberListener (DDS 1.4 §2.2.4.2.3)

```c
typedef struct {
    void* user_data;
    void (*on_data_on_readers)(void*, zerodds_Subscriber*);
    void (*on_sample_lost)(void*, zerodds_DataReader*);
    void (*on_sample_rejected)(void*, zerodds_DataReader*);
    void (*on_liveliness_changed)(void*, zerodds_DataReader*);
    void (*on_subscription_matched)(void*, zerodds_DataReader*);
    void (*on_requested_deadline_missed)(void*, zerodds_DataReader*);
    void (*on_requested_incompatible_qos)(void*, zerodds_DataReader*);
    void (*on_data_available)(void*, zerodds_DataReader*);
} zerodds_SubscriberListener;
```

### §2.4 TopicListener (DDS 1.4 §2.2.4.2.4)

```c
typedef struct {
    void* user_data;
    void (*on_inconsistent_topic)(void*, zerodds_Topic*);
} zerodds_TopicListener;
```

### §2.5 DataWriterListener (DDS 1.4 §2.2.4.2.5)

Siehe §1.1.

### §2.6 DataReaderListener (DDS 1.4 §2.2.4.2.6)

```c
typedef struct {
    void* user_data;
    void (*on_data_available)(void*, zerodds_DataReader*);
    void (*on_sample_rejected)(void*, zerodds_DataReader*);
    void (*on_liveliness_changed)(void*, zerodds_DataReader*);
    void (*on_requested_deadline_missed)(void*, zerodds_DataReader*);
    void (*on_requested_incompatible_qos)(void*, zerodds_DataReader*);
    void (*on_subscription_matched)(void*, zerodds_DataReader*);
    void (*on_sample_lost)(void*, zerodds_DataReader*);
} zerodds_DataReaderListener;
```

## §3 Status-Mask-Semantik (Spec §2.2.4.2.1.4)

`status_mask` ist eine Bitmaske ueber `dds::core::status::StatusKind`.
Ein Callback wird nur dann gefeuert, wenn das entsprechende
Status-Bit gesetzt ist UND der Funktions-Pointer non-NULL ist.

```
StatusMask Bit                     | Callback                        | Entity-Type
-----------------------------------|----------------------------------|------------------
INCONSISTENT_TOPIC          (0x01) | on_inconsistent_topic           | DP, Topic
OFFERED_DEADLINE_MISSED     (0x02) | on_offered_deadline_missed      | DP, Pub, DW
REQUESTED_DEADLINE_MISSED   (0x04) | on_requested_deadline_missed    | DP, Sub, DR
OFFERED_INCOMPATIBLE_QOS    (0x20) | on_offered_incompatible_qos     | DP, Pub, DW
REQUESTED_INCOMPATIBLE_QOS  (0x40) | on_requested_incompatible_qos   | DP, Sub, DR
SAMPLE_LOST                 (0x80) | on_sample_lost                  | DP, Sub, DR
SAMPLE_REJECTED            (0x100) | on_sample_rejected              | DP, Sub, DR
DATA_ON_READERS            (0x200) | on_data_on_readers              | DP, Sub
DATA_AVAILABLE             (0x400) | on_data_available               | DP, Sub, DR
LIVELINESS_LOST            (0x800) | on_liveliness_lost              | DP, Pub, DW
LIVELINESS_CHANGED        (0x1000) | on_liveliness_changed           | DP, Sub, DR
PUBLICATION_MATCHED       (0x2000) | on_publication_matched          | DP, Pub, DW
SUBSCRIPTION_MATCHED      (0x4000) | on_subscription_matched         | DP, Sub, DR
```

`status_mask = 0xFFFFFFFF` aktiviert alle gesetzten Pointer.

## §4 Threading-Vertrag

### §4.1 Async-Delivery

Alle Callbacks werden vom Runtime-Worker-Thread (UDP-RX +
Discovery-Tick) gefeuert, niemals synchron im Caller-Kontext eines
`set_listener`/`write`/`take`-Aufrufs. Spec §2.2.4.0 explizit:
"Listener calls happen in implementation-specific threads".

### §4.2 Re-Entrancy

Caller-Code IM Callback darf:
- DDS-Read-Operations machen (`take`, `read`, `get_qos`).
- DDS-Status-Read machen (`get_publication_matched_status`).

Caller-Code IM Callback darf **NICHT**:
- `set_listener` rufen — fuehrt zu Deadlock-Risiko.
- Den Listener selbst freigeben.

### §4.3 Lifetime

`zerodds_*_set_listener(entity, ptr_to_listener, mask)` speichert nur
einen Pointer; die `Zerodds*Listener`-Struktur bleibt im Besitz des
Callers. Caller MUSS die Struktur am Leben halten bis er
`set_listener(entity, NULL, 0)` ruft oder die Entity geloescht wird.

## §5 Bubble-Up-Pfad (Spec §2.2.4.2.0)

Wenn an einer DataWriter-Entity ein Status-Bit ohne registrierten
Callback feuert, propagiert die Implementation zu Pub-Listener und dann
DP-Listener. Erste Match in Hierarchie gewinnt; nachfolgende Listener
sehen das Event nicht (Spec-Verhalten).

```
DataWriter event → DataWriterListener (bit set + ptr non-NULL)?
                       └─ no → PublisherListener (bit set + ptr non-NULL)?
                                  └─ no → DomainParticipantListener?
```

RC1: DataWriter→Publisher Bubble-Up ist nicht aktiv (siehe §6).

## §6 Status-Phasen (RC1 → Phase-2)

Diese Vendor-Spec wird in 3 Phasen ausgebaut:

### Phase 1 (RC1, 2026-05-06): API-Surface

- Alle 6 Listener-Strukturen sind als `#[repr(C)]` exponiert.
- `*_set_listener` / `*_get_listener` speichern den Pointer in einer
  globalen `OnceLock<Mutex<HashMap>>`.
- `*_get_listener` liefert den letzten gesetzten Pointer.
- **Active-Wireup an die Runtime ist NICHT aktiv.** Callbacks werden
  nicht gefeuert, der Pointer wird nur gespeichert.
- Cross-Language-Bindings (C++/C#/Java/TS/Python) koennen Listener-
  Subclasses bauen und kompilieren — Wirken aber nicht zur Laufzeit.

### Phase 2 (geplant): Active-Wireup

- Runtime-Worker-Thread (siehe `zerodds_dcps::runtime::status_emit_*`)
  prueft beim Status-Counter-Increment ob ein Listener registriert ist
  und feuert den entsprechenden Callback.
- Status-Mask-Filter wird angewendet.
- Bubble-Up-Pfad zu Pub/Sub/DP wird aktiv.

### Phase 3 (geplant): Sub-Aggregator-Sema

- `Subscriber::on_data_on_readers` mit Set-Semantik (kein Duplicate-
  Trigger pro Tick).
- `DomainParticipantListener` als Catch-All fuer alle 13 Status-Bits
  inkl. Bubble-Up.

## §7 Cross-Language-Mapping

### §7.1 C++ (DDS-PSM-Cxx 1.0 §7.5.9)

```cpp
namespace dds::pub {
    template <typename T>
    class DataWriterListener {
    public:
        virtual void on_liveliness_lost(DataWriter<T>&,
                                         const status::LivelinessLostStatus&) = 0;
        // ... 3 mehr
    };
}
```

C++ wrappt die `zerodds_DataWriterListener`-Struct: pro Method ein
`extern "C"` shim, das `user_data` zurueck-cast auf
`DataWriterListener<T>*` und die virtual-Methode ruft.

### §7.2 C# (.NET-Idiom)

```csharp
namespace ZeroDDS.Pub {
    public interface IDataWriterListener<T> {
        void OnLivelinessLost(DataWriter<T> dw, LivelinessLostStatus s);
        // ... 3 mehr
    }
}
```

C#-Wrapper haelt `GCHandle.ToIntPtr(handle)` als `user_data` —
NativeAOT-kompatibel ohne Reflection.

### §7.3 Java (DDS-Java-PSM 1.0 §8.7)

```java
package org.omg.dds.pub;
public interface DataWriterListener<T> extends Listener {
    void onLivelinessLost(DataWriter<T> dw, LivelinessLostStatus status);
    // ... 3 mehr
}
```

JNI-Bridge in `crates/zerodds-java-jni/` waehlt zwischen JNI-Pfad
(legacy) und gRPC-Bridge-Pfad (pure-Java per Vendor-Extension).

### §7.4 Python (PyO3-Idiom)

```python
class DataWriterListener:
    def on_liveliness_lost(self, dw, status): ...
    # ... 3 mehr
```

PyO3-Wrapper haelt `Py<PyAny>` als `user_data`.

### §7.5 TypeScript (Node + WASM)

```typescript
interface DataWriterListener<T> {
  onLivelinessLost(dw: DataWriter<T>, status: LivelinessLostStatus): void;
  // ... 3 mehr
}
```

Koffi-Wrapper traegt eine V8-`weak_ref` als `user_data`.

## §8 Test-Pflicht

Jede Sprach-Binding muss demonstrieren:

1. `set_listener(entity, listener, FULL_MASK)` und anschliessendes
   `get_listener(entity)` liefern denselben Pointer (Identity-Round-trip).
2. `set_listener(entity, NULL, 0)` clears.
3. Listener-Struktur bleibt nach `*_destroy(entity)` referenz-frei (kein
   Use-after-Free in der Registry).

Phase-2 erweitert um Active-Callback-Tests.

## §9 Memory-Ownership

| Operation                          | Owner            |
|------------------------------------|------------------|
| `Zerodds*Listener struct allocated by caller` | Caller   |
| `set_listener(entity, ptr, mask)` | Registry haelt weak Pointer |
| `set_listener(entity, NULL, 0)`   | Registry clear   |
| Caller kills Listener struct       | MUSS vorher `set_listener(NULL)` rufen |

## §10 Stabilitaet

Vendor-Spec, semver:

- v1.0 = aktuelle Surface (RC1).
- Breaking-Changes verlangen v2.0-Major-Bump.
- Phase-2/Phase-3-Erweiterungen sind v1.1+ Backwards-compatible (neue
  Felder am STRUKTUR-ENDE; alte Caller bleiben kompatibel).

## §11 Nicht-spec-konforme Erweiterungen

Keine — alle 17 Spec-Listener-Methoden 1:1 als C-FFI-Funktions-Pointer.
Die `void* user_data`-Konvention ist Vendor-Detail (Spec definiert
Listener als Klassen ohne separates user_data-Feld), aber notwendig
fuer die C-FFI-Abbildung.
