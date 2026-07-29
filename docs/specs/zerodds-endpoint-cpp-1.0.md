# `zerodds-endpoint-cpp` v1.0 — C++17 Endpoint-SDK: XRCE-Framing, Sync/Async, Reliable Stream

ZeroDDS Vendor-Spec. Implementiert in `endpoints/cpp/`. Baut auf
`docs/adr/0013-native-endpoint-sdks.md` (Frame-Hook-Vertrag), DDS-XRCE 1.0 §8.3
(Framing) und [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md) (reliable
Stream) auf. Ergänzt [`zerodds-xcdr2-cpp-1.0`](zerodds-xcdr2-cpp-1.0.md) — dort
der `topic_type_support<T>`-Codec, hier der Endpoint, der ihn transportiert.

## §1 XRCE-Framing

Das SDK ist transport-opak (ADR-0013, Invariante 5): es reicht eine fertig
geframte, kodierte Message an einen integrator-gestellten Transport und
empfängt vollständige Frames zurück.

```c
typedef struct zdw_transport {
    void *ctx;
    int (*deliver)(void *ctx, const unsigned char *frame, size_t len);
    int (*receive)(void *ctx, unsigned char *buf, size_t cap, size_t *len);
} zdw_transport;
```

Der C++-SDK re-implementiert das XRCE-Framing NICHT. Er bindet den
C89-Wire-Core (`endpoints/c/include/zerodds_endpoint.h`) direkt per
`extern "C"` ein, damit er nicht vom byte-identischen Kern abdriften kann.

Rahmen: DDS-XRCE-1.0-Message-Header (8 Byte: session, stream, seq LE) +
WRITE_DATA-Submessage-Header (id `0x07`, flags, len LE) + Sample-Body.
Best-effort Stream `0x01`, kein ClientKey (`session_id >= 128`).

```c
#define ZDW_XRCE_SESSION_NOKEY      0x80
#define ZDW_XRCE_STREAM_BEST_EFFORT 0x01

size_t zdw_xrce_write_frame(unsigned char *out, size_t cap,
                            unsigned char session, unsigned char stream,
                            unsigned int seq, const unsigned char *sample,
                            size_t sample_len);
int zdw_xrce_read_frame(const unsigned char *frame, size_t len,
                        const unsigned char **body, size_t *body_len);
```

`zdw_xrce_write_frame` MUSS den Sample-Body in genau diesem Layout wrappen;
`zdw_xrce_read_frame` MUSS ihn ohne Kopie lokalisieren (`*body` zeigt in den
Frame-Puffer). Header und Submessage-Header sind stets Little-Endian
(Spec §8.3.2.3/§8.3.4); die Byte-Order des Sample-Bodys trägt das E-Flag.

## §2 sync Client

Ein blockierender Transport-Poll-Client über denselben `zdw_transport`:

```c
int zdw_endpoint_send(const zdw_transport *t, const unsigned char *frame, size_t len);
int zdw_endpoint_recv(const zdw_transport *t, unsigned char *buf, size_t cap, size_t *len);
```

Für das Marshalling des Sample-Bodys stellt `endpoints/cpp/include/zerodds_wire.hpp`
eine C++98-Fassade (`zerodds::Writer` / `zerodds::Reader`) über den C89-Kern
bereit — kein C++11+, kein Boost, minimale STL (nur `std::string`):

```cpp
namespace zerodds {

class Writer {
public:
    Writer(unsigned char* buf, size_t cap, int endian);
    int u8(unsigned char v);
    int u16(unsigned int v);
    int u32(unsigned long v);
    int u64(zdw_u64_t v);
    int boolean(bool v);
    int f32(float v);
    int f64(double v);
    int str(const std::string& s);
    int seq_u8(const unsigned char* d, size_t n);
    size_t dheader_begin();
    int dheader_end(size_t body_start);
    size_t emheader_begin(unsigned long member_id, bool must_understand);
    int emheader_end(size_t body_start);
    size_t size() const;
    int error() const;
};

class Reader {
public:
    Reader(const unsigned char* buf, size_t len, int endian);
    unsigned char u8();
    unsigned int  u16();
    unsigned long u32();
    zdw_u64_t     u64();
    bool          boolean();
    float         f32();
    double        f64();
    std::string   str();
    size_t seq_u8(unsigned char* out, size_t cap);
    unsigned long dheader();
    unsigned long emheader(unsigned long* nextint);
    int error() const;
};

} // namespace zerodds
```

Ein sync App-Flow MUSS: (1) den Sample-Body per `Writer` marshaln, (2) über
`zdw_xrce_write_frame` framen, (3) über `t.deliver` liefern; auf der
Empfangsseite (4) über `t.receive` pollen, (5) über `zdw_xrce_read_frame`
deframen, (6) den Body per `Reader` unmarshaln. Das SDK selbst führt keinen
eigenen Poll-Loop — der Integrator besitzt die Schleife (Poll-Intervall,
Abbruchbedingung sind Anwendungsentscheidung).

## §3 async Reader/Writer

Ein event-driven, nicht-blockierender Reader (Callback pro empfangenem
Sample) und ein Fire-and-forget-Writer, additiv zur konservativen
C++98-Wire-Fassade — dünne C++17-Fassaden über den auditierten C-Reaktor
(`endpoints/c/src/zerodds_async.c`):

```cpp
namespace zerodds {

class AsyncReader {
public:
    using OnSample = std::function<void(const unsigned char* body, std::size_t len)>;

    AsyncReader(const zdw_transport* transport, unsigned char* rxbuf,
                std::size_t rxcap, OnSample on_sample);
    AsyncReader(const AsyncReader&) = delete;
    AsyncReader& operator=(const AsyncReader&) = delete;

    int poll();          // ZDW_T_OK / ZDW_T_AGAIN / ZDW_T_ERROR
    int run(int max = 0); // drains bis ZDW_T_AGAIN oder max Frames; liefert Count
};

class AsyncWriter {
public:
    AsyncWriter(const zdw_transport* transport, unsigned char* txbuf,
                std::size_t txcap, unsigned char session, unsigned char stream);

    bool write(const unsigned char* sample, std::size_t len); // true = delivered
};

}  // namespace zerodds
```

`AsyncReader` MUSS jedes vollständige, deframete Sample über den
`std::function`-Trampolin (`AsyncReader::trampoline`) an `on_sample`
dispatchen, RAII-verwaltet, ohne Copy/Move (non-copyable). `AsyncWriter`
framet und liefert synchron innerhalb von `write()` — "async" heißt hier
nicht-blockierend beim Empfang (Reader), nicht Entkopplung des Schreibers;
die Entkopplung des Producers vom I/O ist die Aufgabe des reliable
`AsyncWriter` in §4.

## §4 Reliable Stream

Normativ: [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md). Dieser
Abschnitt bindet den kanonischen State-Machine-Kontrakt (§3 dort) an die
C++17-API.

`endpoints/cpp/include/zerodds_reliable.hpp` ist header-only C++17
(`zerodds::reliable`-Namespace), byte-identisch zu `crates/xrce` +
`endpoints/c` im Wire-Layout:

```cpp
namespace zerodds { namespace reliable {

constexpr unsigned    HEARTBEAT_PERIOD_MS = 500;
constexpr std::size_t SENDER_WINDOW       = 16;
constexpr std::size_t RECEIVER_BUFFER     = 64;
constexpr std::size_t MAX_PAYLOAD         = 65535;

enum class SubmitErr { Ok, WindowFull, PayloadTooLarge };
enum class RecvErr   { Ok, BufferFull };

class Sender {
public:
    SubmitErr submit(const Bytes& payload, std::uint16_t& out_seq);
    std::optional<Heartbeat> pending_heartbeat(std::uint64_t now_ms);
    void recv_acknack(std::uint16_t base, std::uint16_t bitmap);
    const Bytes* get_in_flight(std::uint16_t seq) const;
};

class Receiver {
public:
    RecvErr recv_data(std::uint16_t seq, Bytes payload);
    std::vector<std::pair<std::uint16_t, Bytes>> drain_in_order();
    AckNack pending_acknack(std::optional<std::uint16_t> hint) const;
    void reset();
};

class AsyncWriter {
public:
    AsyncWriter(SendBatch send_batch, SendOne send_one, PollAck poll_ack,
                std::size_t ring_cap = 512);
    bool enqueue(const std::uint8_t* data, std::size_t n); // wait-free
    void finish(); // drain bis Fenster+Ring leer, dann stop
    void stop();   // unconditional teardown (responder-lose Kontexte)
};

}} // namespace zerodds::reliable
```

`Sender`/`Receiver` MÜSSEN exakt die Methoden aus
`reliable-endpoint-1.0` §3.2/§3.3 spiegeln (idiomatischer C++-Name, gleiche
Semantik): `submit`, `pending_heartbeat`, `recv_acknack`, `get_in_flight`
bzw. `recv_data`, `drain_in_order`, `pending_acknack`, `reset`.

Der `AsyncWriter` ist das in `reliable-endpoint-1.0` §2 beschriebene Bauteil:
`enqueue()` schreibt wait-free in einen Lock-freien SPSC-Ring
(`std::atomic<std::size_t> head_`/`tail_`, kein Syscall, kein Lock); ein
dedizierter `std::thread`-Drain-Task besitzt den `Sender`-State exklusiv,
holt reife Samples aus dem Ring, sendet sie gebündelt per `send_batch_`,
feuert `HEARTBEAT`s periodisch, verarbeitet `ACKNACK`s per `poll_ack_` und
retransmittet die noch fehlenden Sequenznummern. Der Producer-Thread darf
den Kernel nie betreten.

Wire-Codec (byte-golden, siehe `reliable-endpoint-1.0` §4):
`write_frame`/`unframe` (WRITE_DATA, id `0x07`), `acknack_frame`/
`parse_acknack` (id `0x0A`), `heartbeat_frame`/`parse_heartbeat` (id `0x0B`).

## §5 Test- und Beleg-Pflicht

Wie in `reliable-endpoint-1.0` §5 vorgeschrieben: Unit (State-Machine),
Byte-Golden (HEARTBEAT/ACKNACK gegen `golden_*.bin`), E2E-Loss-Recovery
gegen den Rust-Referenz-Peer, Latenz-Bench (`enqueue` vs. inline `sendto`),
und ein lauffähiges `example_reliable_*`. Kein false-green: lauter Skip nur
bei fehlender Toolchain (`g++`/`gcc` nicht auf `PATH`).
