# `zerodds-endpoint-cpp` v1.0 — C++17 endpoint SDK: XRCE framing, sync/async, reliable stream

ZeroDDS vendor spec. Implemented in `endpoints/cpp/`. Builds on
`docs/adr/0013-native-endpoint-sdks.md` (frame hook contract), DDS-XRCE 1.0 §8.3
(framing) and [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md) (reliable
stream). Complements [`zerodds-xcdr2-cpp-1.0`](zerodds-xcdr2-cpp-1.0.md) — there
the `topic_type_support<T>` codec, here the endpoint that transports it.

## §1 XRCE framing

The SDK is transport-opaque (ADR-0013, invariant 5): it hands a fully
framed, encoded message to an integrator-provided transport and
receives complete frames back.

```c
typedef struct zdw_transport {
    void *ctx;
    int (*deliver)(void *ctx, const unsigned char *frame, size_t len);
    int (*receive)(void *ctx, unsigned char *buf, size_t cap, size_t *len);
} zdw_transport;
```

The C++ SDK does NOT re-implement the XRCE framing. It includes the
C89 wire core (`endpoints/c/include/zerodds_endpoint.h`) directly via
`extern "C"`, so that it cannot drift away from the byte-identical core.

Frame: DDS-XRCE 1.0 message header (8 bytes: session, stream, seq LE) +
WRITE_DATA submessage header (id `0x07`, flags, len LE) + sample body.
Best-effort stream `0x01`, no ClientKey (`session_id >= 128`).

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

`zdw_xrce_write_frame` MUST wrap the sample body in exactly this layout;
`zdw_xrce_read_frame` MUST locate it without a copy (`*body` points into the
frame buffer). Header and submessage header are always little-endian
(spec §8.3.2.3/§8.3.4); the byte order of the sample body is carried by the E flag.

The submessage body is the `BaseObjectRequest` (DDS-XRCE 1.0 §7.7.8:
`request_id` then `object_id`, two octets each, most significant first),
followed by the sample. The flag byte is `0x01` — E flag (little-endian body)
plus `DataFormat` `FORMAT_DATA` (`0b000`) in bits 1..3 (§8.3.5.8);
`FORMAT_SAMPLE` (`0b001`, flag byte `0x03`) additionally requires a
`SampleInfo`, which this profile does not carry. The `request_id` is the
`sequence_nr` of the same message (§7.7.8 only requires uniqueness per
stream), the `object_id` names the target DataWriter — without it no
XRCE agent can assign the frame. The read path returns the sample after these
four octets and rejects a body that is too short for them.
Measured by `crates/endpoint-e2e/tests/xrce_write_framing.rs`.

## §2 sync client

A blocking transport poll client over the same `zdw_transport`:

```c
int zdw_endpoint_send(const zdw_transport *t, const unsigned char *frame, size_t len);
int zdw_endpoint_recv(const zdw_transport *t, unsigned char *buf, size_t cap, size_t *len);
```

For marshalling the sample body, `endpoints/cpp/include/zerodds_wire.hpp` provides
a C++98 facade (`zerodds::Writer` / `zerodds::Reader`) over the C89 core
— no C++11+, no Boost, minimal STL (only `std::string`):

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

A sync app flow MUST: (1) marshal the sample body via `Writer`, (2) frame it via
`zdw_xrce_write_frame`, (3) deliver it via `t.deliver`; on the
receiving side (4) poll via `t.receive`, (5) deframe via `zdw_xrce_read_frame`,
(6) unmarshal the body via `Reader`. The SDK itself runs no
poll loop of its own — the integrator owns the loop (poll interval and
termination condition are an application decision).

## §3 async reader/writer

An event-driven, non-blocking reader (callback per received
sample) and a fire-and-forget writer, in addition to the conservative
C++98 wire facade — thin C++17 facades over the audited C reactor
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
    int run(int max = 0); // drains until ZDW_T_AGAIN or max frames; returns count
};

class AsyncWriter {
public:
    AsyncWriter(const zdw_transport* transport, unsigned char* txbuf,
                std::size_t txcap, unsigned char session, unsigned char stream);

    bool write(const unsigned char* sample, std::size_t len); // true = delivered
};

}  // namespace zerodds
```

`AsyncReader` MUST dispatch every complete, deframed sample via the
`std::function` trampoline (`AsyncReader::trampoline`) to `on_sample`,
RAII-managed, without copy/move (non-copyable). `AsyncWriter`
frames and delivers synchronously within `write()` — "async" here means
non-blocking on receive (reader), not decoupling of the writer;
decoupling the producer from the I/O is the task of the reliable
`AsyncWriter` in §4.

## §4 Reliable stream

Normative: [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md). This
section binds the canonical state machine contract (§3 there) to the
C++17 API.

`endpoints/cpp/include/zerodds_reliable.hpp` is header-only C++17
(`zerodds::reliable` namespace), byte-identical to `crates/xrce` +
`endpoints/c` in the wire layout:

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
    void finish(); // drain until window+ring empty, then stop
    void stop();   // unconditional teardown (contexts without a responder)
};

}} // namespace zerodds::reliable
```

`Sender`/`Receiver` MUST mirror exactly the methods from
`reliable-endpoint-1.0` §3.2/§3.3 (idiomatic C++ name, same
semantics): `submit`, `pending_heartbeat`, `recv_acknack`, `get_in_flight`
and `recv_data`, `drain_in_order`, `pending_acknack`, `reset` respectively.

The `AsyncWriter` is the component described in `reliable-endpoint-1.0` §2:
`enqueue()` writes wait-free into a lock-free SPSC ring
(`std::atomic<std::size_t> head_`/`tail_`, no syscall, no lock); a
dedicated `std::thread` drain task owns the `Sender` state exclusively,
fetches ready samples from the ring, sends them batched via `send_batch_`,
fires `HEARTBEAT`s periodically, processes `ACKNACK`s via `poll_ack_` and
retransmits the sequence numbers that are still missing. The producer thread must
never enter the kernel.

Wire codec (byte-golden, see `reliable-endpoint-1.0` §4):
`write_frame`/`unframe` (WRITE_DATA, id `0x07`), `acknack_frame`/
`parse_acknack` (id `0x0A`), `heartbeat_frame`/`parse_heartbeat` (id `0x0B`).

## §5 Test and evidence obligation

As prescribed in `reliable-endpoint-1.0` §5: unit (state machine),
byte-golden (HEARTBEAT/ACKNACK against `golden_*.bin`), E2E loss recovery
against the Rust reference peer, latency bench (`enqueue` vs. inline `sendto`),
and a runnable `example_reliable_*`. No false green: a loud skip only
when the toolchain is missing (`g++`/`gcc` not on `PATH`).
