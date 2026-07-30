// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Reliable XRCE stream (DDS-XRCE 1.0 §8.4.10/§8.4.11) for the C++ endpoint SDK.
// Header-only C++17. Byte-identical to `crates/xrce` + `endpoints/c`:
//   frame = [session][stream][seq u16 LE][submsg id][flags][len u16 LE][body]
//   WRITE_DATA id 0x07 flags 0x03; ACKNACK id 0x0A flags 0x01 body(5);
//   HEARTBEAT id 0x0B flags 0x01 body(5). Sequence numbers are RFC-1982 16-bit.
//
// Two roles, mirroring the reference `ReliableStreamState`:
//   Sender   — submit / pending_heartbeat / recv_acknack / get_in_flight
//   Receiver — recv_data / drain_in_order / pending_acknack / reset
//
// `AsyncWriter` is the async-decoupled writer: the producer `enqueue()`s
// wait-free into a lock-free SPSC ring (ns, no syscall) and a drain thread holds
// the reliable state, batches WRITE_DATA via the supplied send-batch hook,
// emits HEARTBEATs, and retransmits on ACKNACK. The producer never enters the
// kernel — the point of async-write at ns cadence.

#ifndef ZERODDS_RELIABLE_HPP
#define ZERODDS_RELIABLE_HPP

#include <atomic>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <map>
#include <optional>
#include <thread>
#include <utility>
#include <vector>

namespace zerodds {
namespace reliable {

// ---- binding constants (see reliable-endpoint-1.0 spec §3) ----
constexpr unsigned    HEARTBEAT_PERIOD_MS = 500;
constexpr std::size_t SENDER_WINDOW       = 16;
constexpr std::size_t RECEIVER_BUFFER     = 64;
constexpr std::size_t MAX_PAYLOAD         = 65535;

constexpr std::uint8_t SESSION_NOKEY   = 0x80;
constexpr std::uint8_t STREAM_RELIABLE = 0x80;  // bit7 set (spec §8.3.2.2)
constexpr std::uint8_t STREAM_NONE     = 0x00;  // control msgs (spec §8.3.5.11)
constexpr std::uint8_t SM_WRITE_DATA   = 0x07;
constexpr std::uint8_t SM_ACKNACK      = 0x0A;
constexpr std::uint8_t SM_HEARTBEAT    = 0x0B;
constexpr std::uint8_t WRITE_FLAGS     = 0x03;
constexpr std::uint8_t FLAG_E_LE       = 0x01;

using Bytes = std::vector<std::uint8_t>;

// ---- RFC-1982 16-bit serial-number comparison ----
inline bool seq_lt(std::uint16_t a, std::uint16_t b) {
    std::uint16_t d = static_cast<std::uint16_t>(b - a);
    return d != 0 && d < 0x8000;
}
inline bool seq_gt(std::uint16_t a, std::uint16_t b) { return seq_lt(b, a); }

struct Heartbeat {
    std::int16_t  first_unacked;
    std::int16_t  last_unacked;
    std::uint8_t  stream;
};
struct AckNack {
    std::int16_t  first_unacked;
    std::uint16_t bitmap;
    std::uint8_t  stream;
};

enum class SubmitErr { Ok, WindowFull, PayloadTooLarge };
enum class RecvErr   { Ok, BufferFull };

// ================== wire codec (byte-golden) ==================

inline Bytes write_frame(std::uint16_t seq, const std::uint8_t* sample, std::size_t n) {
    Bytes out;
    out.reserve(8 + n);
    out.push_back(SESSION_NOKEY);
    out.push_back(STREAM_RELIABLE);
    out.push_back(static_cast<std::uint8_t>(seq & 0xFF));
    out.push_back(static_cast<std::uint8_t>((seq >> 8) & 0xFF));
    out.push_back(SM_WRITE_DATA);
    out.push_back(WRITE_FLAGS);
    out.push_back(static_cast<std::uint8_t>(n & 0xFF));
    out.push_back(static_cast<std::uint8_t>((n >> 8) & 0xFF));
    out.insert(out.end(), sample, sample + n);
    return out;
}

// Returns {seq, offset-of-body, body-len} if `frame` is a reliable WRITE_DATA.
inline std::optional<std::pair<std::uint16_t, std::pair<std::size_t, std::size_t>>>
unframe(const std::uint8_t* frame, std::size_t len) {
    if (len < 8 || frame[4] != SM_WRITE_DATA) return std::nullopt;
    std::uint16_t seq = static_cast<std::uint16_t>(frame[2] | (frame[3] << 8));
    return std::make_pair(seq, std::make_pair(std::size_t{8}, len - 8));
}

// Explicit header fields so a test can reproduce the golden exactly
// (golden uses hdr_stream=STREAM_NONE, hdr_seq=1); the live peer path uses
// hdr_stream=STREAM_RELIABLE, hdr_seq=0.
inline Bytes acknack_frame(std::uint8_t session, std::uint8_t hdr_stream,
                           std::uint16_t hdr_seq, std::int16_t first_unacked,
                           std::uint8_t nack_lo, std::uint8_t nack_hi,
                           std::uint8_t payload_stream) {
    std::uint16_t fu = static_cast<std::uint16_t>(first_unacked);
    return Bytes{session,
                 hdr_stream,
                 static_cast<std::uint8_t>(hdr_seq & 0xFF),
                 static_cast<std::uint8_t>((hdr_seq >> 8) & 0xFF),
                 SM_ACKNACK,
                 FLAG_E_LE,
                 5,
                 0,
                 static_cast<std::uint8_t>(fu & 0xFF),
                 static_cast<std::uint8_t>((fu >> 8) & 0xFF),
                 nack_lo,
                 nack_hi,
                 payload_stream};
}

inline Bytes heartbeat_frame(std::uint8_t session, std::uint8_t hdr_stream,
                             std::uint16_t hdr_seq, std::int16_t first,
                             std::int16_t last, std::uint8_t payload_stream) {
    std::uint16_t f = static_cast<std::uint16_t>(first);
    std::uint16_t l = static_cast<std::uint16_t>(last);
    return Bytes{session,
                 hdr_stream,
                 static_cast<std::uint8_t>(hdr_seq & 0xFF),
                 static_cast<std::uint8_t>((hdr_seq >> 8) & 0xFF),
                 SM_HEARTBEAT,
                 FLAG_E_LE,
                 5,
                 0,
                 static_cast<std::uint8_t>(f & 0xFF),
                 static_cast<std::uint8_t>((f >> 8) & 0xFF),
                 static_cast<std::uint8_t>(l & 0xFF),
                 static_cast<std::uint8_t>((l >> 8) & 0xFF),
                 payload_stream};
}

inline std::optional<AckNack> parse_acknack(const std::uint8_t* f, std::size_t len) {
    if (len < 13 || f[4] != SM_ACKNACK) return std::nullopt;
    return AckNack{static_cast<std::int16_t>(f[8] | (f[9] << 8)),
                   static_cast<std::uint16_t>(f[10] | (f[11] << 8)), f[12]};
}

inline std::optional<Heartbeat> parse_heartbeat(const std::uint8_t* f, std::size_t len) {
    if (len < 13 || f[4] != SM_HEARTBEAT) return std::nullopt;
    return Heartbeat{static_cast<std::int16_t>(f[8] | (f[9] << 8)),
                     static_cast<std::int16_t>(f[10] | (f[11] << 8)), f[12]};
}

// ================== sender state machine ==================

class Sender {
public:
    SubmitErr submit(const Bytes& payload, std::uint16_t& out_seq) {
        if (payload.size() > MAX_PAYLOAD) return SubmitErr::PayloadTooLarge;
        if (in_flight_.size() >= SENDER_WINDOW) return SubmitErr::WindowFull;
        out_seq = next_seq_;
        in_flight_[next_seq_] = payload;
        next_seq_ = static_cast<std::uint16_t>(next_seq_ + 1);
        return SubmitErr::Ok;
    }

    std::optional<Heartbeat> pending_heartbeat(std::uint64_t now_ms) {
        if (in_flight_.empty()) return std::nullopt;
        bool due = !last_hb_.has_value() || (now_ms - *last_hb_) >= HEARTBEAT_PERIOD_MS;
        if (!due) return std::nullopt;
        last_hb_ = now_ms;
        // RFC-1982 window base (oldest unacked) + end (newest unacked); NOT the
        // numeric map begin()/rbegin(), which are wrong across a 16-bit wrap
        // (window 0xFFFE,0xFFFF,0x0000,0x0001 → base 0xFFFE / end 0x0001, not
        // 0x0000 / 0xFFFF). Mirrors window_base / serial_max_in_flight in
        // crates/xrce/src/reliable.rs.
        std::uint16_t first = 0, last = 0;
        bool seen = false;
        for (const auto& kv : in_flight_) {
            std::uint16_t k = kv.first;
            if (!seen) {
                first = k;
                last = k;
                seen = true;
            } else {
                if (seq_lt(k, first)) first = k;
                if (seq_gt(k, last)) last = k;
            }
        }
        return Heartbeat{static_cast<std::int16_t>(first),
                         static_cast<std::int16_t>(last), STREAM_RELIABLE};
    }

    void recv_acknack(std::uint16_t base, std::uint16_t bitmap) {
        std::vector<std::uint16_t> rm;
        for (const auto& kv : in_flight_) {
            std::uint16_t d = static_cast<std::uint16_t>(base - kv.first);
            if (d != 0 && d < 0x8000) rm.push_back(kv.first);  // key < base
        }
        for (auto k : rm) in_flight_.erase(k);
        for (std::uint16_t i = 0; i < 16; ++i) {
            std::uint16_t seq = static_cast<std::uint16_t>(base + i);
            if (((bitmap >> i) & 1u) == 0) in_flight_.erase(seq);  // acked
        }
    }

    const Bytes* get_in_flight(std::uint16_t seq) const {
        auto it = in_flight_.find(seq);
        return it == in_flight_.end() ? nullptr : &it->second;
    }
    std::size_t in_flight_count() const { return in_flight_.size(); }
    const std::map<std::uint16_t, Bytes>& in_flight() const { return in_flight_; }

private:
    std::uint16_t next_seq_ = 0;
    std::map<std::uint16_t, Bytes> in_flight_;
    std::optional<std::uint64_t> last_hb_;
};

// ================== receiver state machine ==================

class Receiver {
public:
    RecvErr recv_data(std::uint16_t seq, Bytes payload) {
        if (seq_lt(seq, expected_)) return RecvErr::Ok;         // duplicate
        if (received_.count(seq)) return RecvErr::Ok;           // already buffered
        if (received_.size() >= RECEIVER_BUFFER) return RecvErr::BufferFull;
        received_[seq] = std::move(payload);
        return RecvErr::Ok;
    }

    std::vector<std::pair<std::uint16_t, Bytes>> drain_in_order() {
        std::vector<std::pair<std::uint16_t, Bytes>> out;
        for (;;) {
            auto it = received_.find(expected_);
            if (it == received_.end()) break;
            out.emplace_back(expected_, std::move(it->second));
            received_.erase(it);
            expected_ = static_cast<std::uint16_t>(expected_ + 1);
        }
        return out;
    }

    AckNack pending_acknack(std::optional<std::uint16_t> hint) const {
        std::uint16_t base = expected_;
        std::uint16_t bitmap = 0;
        for (std::uint16_t i = 0; i < 16; ++i) {
            std::uint16_t seq = static_cast<std::uint16_t>(base + i);
            if (hint && seq_gt(seq, *hint)) continue;
            if (!received_.count(seq)) bitmap = static_cast<std::uint16_t>(bitmap | (1u << i));
        }
        return AckNack{static_cast<std::int16_t>(base), bitmap, STREAM_RELIABLE};
    }

    void reset() {
        expected_ = 0;
        received_.clear();
    }
    std::uint16_t expected() const { return expected_; }
    std::size_t out_of_order_count() const { return received_.size(); }

private:
    std::uint16_t expected_ = 0;
    std::map<std::uint16_t, Bytes> received_;
};

// ================== async-decoupled writer ==================
//
// Producer thread: `enqueue()` is wait-free (write a slot + release the tail);
// it never touches the socket. The drain thread owns the `Sender`, batches
// ready WRITE_DATA frames through `send_batch`, times HEARTBEATs, feeds incoming
// ACKNACKs into `recv_acknack`, and retransmits the still-missing samples.

class AsyncWriter {
public:
    using SendBatch = std::function<void(const std::vector<Bytes>&)>;
    using SendOne   = std::function<void(const Bytes&)>;
    using PollAck   = std::function<int(std::uint8_t*, std::size_t)>;  // >=0 len, -1 none

    AsyncWriter(SendBatch send_batch, SendOne send_one, PollAck poll_ack,
                std::size_t ring_cap = 512,
                std::chrono::milliseconds drain_deadline = std::chrono::milliseconds(2000))
        : send_batch_(std::move(send_batch)),
          send_one_(std::move(send_one)),
          poll_ack_(std::move(poll_ack)),
          cap_(ring_cap),
          ring_(ring_cap),
          drain_deadline_(drain_deadline) {
        drain_ = std::thread([this] { drain_loop(); });
    }

    // Destruction is bounded: finish() enters the drain phase and the drain
    // thread self-terminates once the window+ring drain OR `drain_deadline_`
    // elapses — an unacked window (dead peer) can no longer wedge the join.
    ~AsyncWriter() {
        finish();
        if (drain_.joinable()) drain_.join();
    }

    // Wait-free enqueue. Returns false if the ring is full (backpressure —
    // the producer decides locally). No syscall, no lock.
    bool enqueue(const std::uint8_t* data, std::size_t n) {
        std::size_t t = tail_.load(std::memory_order_relaxed);
        std::size_t next = (t + 1) % cap_;
        if (next == head_.load(std::memory_order_acquire)) return false;  // full
        ring_[t].assign(data, data + n);
        tail_.store(next, std::memory_order_release);
        return true;
    }

    // Producer signals it is done; the drain thread runs until the window and
    // the ring are both empty, then stops.
    void finish() { finished_.store(true, std::memory_order_release); }

    // Unconditional teardown: the drain thread exits at the next iteration even
    // if the window still holds unacked samples. For responder-less contexts
    // (e.g. a producer-latency bench) where the window can never drain.
    void stop() { abort_.store(true, std::memory_order_release); }

    // For the latency baseline: the inline path a naive writer would take.
    void inline_send(std::uint16_t seq, const std::uint8_t* data, std::size_t n) {
        send_one_(write_frame(seq, data, n));
    }

    // Samples dropped because they exceed MAX_PAYLOAD (skipped, never sent —
    // they must not wedge the ring head waiting for a slot they can never use).
    std::size_t dropped() const { return dropped_.load(std::memory_order_relaxed); }

private:
    static std::uint64_t now_ms() {
        return static_cast<std::uint64_t>(
            std::chrono::duration_cast<std::chrono::milliseconds>(
                std::chrono::steady_clock::now().time_since_epoch())
                .count());
    }

    void drain_loop() {
        std::uint8_t rx[256];
        std::chrono::steady_clock::time_point deadline{};
        bool have_deadline = false;
        for (;;) {
            if (abort_.load(std::memory_order_acquire)) return;
            // 1) pull ready samples from the ring into the sender window.
            std::vector<Bytes> batch;
            std::size_t h = head_.load(std::memory_order_relaxed);
            while (h != tail_.load(std::memory_order_acquire) &&
                   sender_.in_flight_count() < SENDER_WINDOW) {
                std::uint16_t seq = 0;
                SubmitErr e = sender_.submit(ring_[h], seq);
                if (e == SubmitErr::WindowFull) {
                    break;  // leave the sample in the ring; an ACKNACK frees a slot
                }
                if (e == SubmitErr::PayloadTooLarge) {
                    // Unsendable: skip it (advance the head) so an oversized sample
                    // at the queue head cannot block every sample behind it forever.
                    dropped_.fetch_add(1, std::memory_order_relaxed);
                    h = (h + 1) % cap_;
                    head_.store(h, std::memory_order_release);
                    continue;
                }
                batch.push_back(write_frame(seq, ring_[h].data(), ring_[h].size()));
                h = (h + 1) % cap_;
                head_.store(h, std::memory_order_release);
            }
            if (!batch.empty()) send_batch_(batch);

            // 2) HEARTBEAT if the period has elapsed.
            if (auto hb = sender_.pending_heartbeat(now_ms())) {
                send_one_(heartbeat_frame(SESSION_NOKEY, STREAM_RELIABLE, 0,
                                          hb->first_unacked, hb->last_unacked,
                                          hb->stream));
            }

            // 3) drain ACKNACKs → clear acked, retransmit the still-missing.
            int n = poll_ack_(rx, sizeof(rx));
            if (n >= 13) {
                if (auto ack = parse_acknack(rx, static_cast<std::size_t>(n))) {
                    std::uint16_t base = static_cast<std::uint16_t>(ack->first_unacked);
                    sender_.recv_acknack(base, ack->bitmap);
                    std::vector<Bytes> retx;
                    for (std::uint16_t i = 0; i < 16; ++i) {
                        if (((ack->bitmap >> i) & 1u) == 0) continue;
                        std::uint16_t seq = static_cast<std::uint16_t>(base + i);
                        if (const Bytes* p = sender_.get_in_flight(seq)) {
                            retx.push_back(write_frame(seq, p->data(), p->size()));
                        }
                    }
                    if (!retx.empty()) send_batch_(retx);
                }
            }

            // 4) termination. Once the producer is done, exit when the window +
            //    ring are fully drained — but never wait longer than the bounded
            //    drain deadline, so an unacked window (dead peer) cannot hang the
            //    destructor's join.
            if (finished_.load(std::memory_order_acquire)) {
                bool empty = sender_.in_flight_count() == 0 &&
                             head_.load(std::memory_order_relaxed) ==
                                 tail_.load(std::memory_order_acquire);
                if (empty) return;
                if (!have_deadline) {
                    deadline = std::chrono::steady_clock::now() + drain_deadline_;
                    have_deadline = true;
                }
                if (std::chrono::steady_clock::now() >= deadline) return;
            }
            std::this_thread::sleep_for(std::chrono::microseconds(200));
        }
    }

    SendBatch send_batch_;
    SendOne   send_one_;
    PollAck   poll_ack_;
    std::size_t cap_;
    std::vector<Bytes> ring_;
    std::atomic<std::size_t> head_{0};
    std::atomic<std::size_t> tail_{0};
    std::atomic<bool> finished_{false};
    std::atomic<bool> abort_{false};
    std::atomic<std::size_t> dropped_{0};
    std::chrono::milliseconds drain_deadline_;
    std::thread drain_;
    Sender sender_;  // owned by the drain thread only
};

}  // namespace reliable
}  // namespace zerodds

#endif  // ZERODDS_RELIABLE_HPP
