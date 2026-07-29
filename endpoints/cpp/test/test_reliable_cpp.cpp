// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Unit tests (mirroring crates/xrce/src/reliable.rs), byte-golden assertions
// against the Rust-generated HEARTBEAT/ACKNACK goldens, and a producer-latency
// micro-bench (decoupled enqueue vs inline send).
//
//   usage: test_reliable_cpp <golden_heartbeat_le.bin> <golden_acknack_le.bin>

#include "zerodds_reliable.hpp"

#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <vector>

#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <unistd.h>

using namespace zerodds::reliable;

static int g_fail = 0;
#define CHECK(cond)                                                      \
    do {                                                                 \
        if (!(cond)) {                                                   \
            std::fprintf(stderr, "FAIL %s:%d  %s\n", __FILE__, __LINE__, #cond); \
            ++g_fail;                                                    \
        }                                                                \
    } while (0)

static Bytes read_file(const char* path) {
    std::ifstream f(path, std::ios::binary);
    return Bytes((std::istreambuf_iterator<char>(f)), std::istreambuf_iterator<char>());
}

// ---- sender ----
static void test_submit_monotonic() {
    Sender s;
    std::uint16_t a = 99, b = 99;
    CHECK(s.submit(Bytes{1, 2}, a) == SubmitErr::Ok);
    CHECK(s.submit(Bytes{3, 4}, b) == SubmitErr::Ok);
    CHECK(a == 0 && b == 1);
    CHECK(s.in_flight_count() == 2);
}
static void test_submit_payload_too_large() {
    Sender s;
    std::uint16_t seq = 0;
    CHECK(s.submit(Bytes(MAX_PAYLOAD + 1, 0), seq) == SubmitErr::PayloadTooLarge);
}
static void test_submit_window_full() {
    Sender s;
    std::uint16_t seq = 0;
    for (std::size_t i = 0; i < SENDER_WINDOW; ++i) CHECK(s.submit(Bytes{0}, seq) == SubmitErr::Ok);
    CHECK(s.submit(Bytes{0}, seq) == SubmitErr::WindowFull);
}
static void test_heartbeat() {
    Sender s;
    std::uint16_t seq = 0;
    CHECK(!s.pending_heartbeat(0).has_value());  // empty window
    s.submit(Bytes{1}, seq);
    auto hb = s.pending_heartbeat(0);
    CHECK(hb && hb->first_unacked == 0 && hb->last_unacked == 0 && hb->stream == 0x80);
    CHECK(!s.pending_heartbeat(100).has_value());  // < 500ms
    CHECK(s.pending_heartbeat(600).has_value());   // >= 500ms
}
static void test_recv_acknack_clears() {
    Sender s;
    std::uint16_t seq = 0;
    for (int i = 0; i < 3; ++i) s.submit(Bytes{static_cast<std::uint8_t>(0xA0 + i)}, seq);
    // base=2, bitmap bit0 set → seq2 missing, 0+1 acked.
    s.recv_acknack(2, 0x0001);
    CHECK(s.in_flight_count() == 1);
    CHECK(s.get_in_flight(2) != nullptr);
}
static void test_recv_acknack_full_clear() {
    Sender s;
    std::uint16_t seq = 0;
    for (int i = 0; i < 5; ++i) s.submit(Bytes{0}, seq);
    s.recv_acknack(5, 0x0000);
    CHECK(s.in_flight_count() == 0);
}

// ---- receiver ----
static void test_recv_in_order() {
    Receiver r;
    r.recv_data(0, Bytes{10});
    r.recv_data(1, Bytes{11});
    auto d = r.drain_in_order();
    CHECK(d.size() == 2 && d[0].first == 0 && d[1].first == 1);
    CHECK(r.expected() == 2);
}
static void test_recv_reorder() {
    Receiver r;
    r.recv_data(2, Bytes{22});
    r.recv_data(0, Bytes{20});
    CHECK(r.drain_in_order().size() == 1);  // only 0
    r.recv_data(1, Bytes{21});
    auto d = r.drain_in_order();
    CHECK(d.size() == 2 && d[0].first == 1 && d[1].first == 2);
}
static void test_recv_duplicate_drop() {
    Receiver r;
    r.recv_data(0, Bytes{1});
    r.drain_in_order();
    r.recv_data(0, Bytes{99});  // < expected → dropped
    CHECK(r.out_of_order_count() == 0);
}
static void test_recv_buffer_full() {
    Receiver r;
    for (std::uint16_t i = 1; i <= RECEIVER_BUFFER; ++i) CHECK(r.recv_data(i, Bytes{1}) == RecvErr::Ok);
    CHECK(r.recv_data(RECEIVER_BUFFER + 1, Bytes{1}) == RecvErr::BufferFull);
}
static void test_pending_acknack_bitmap() {
    Receiver r;
    r.recv_data(1, Bytes{1});
    r.recv_data(3, Bytes{3});
    auto ack = r.pending_acknack(std::optional<std::uint16_t>(3));
    CHECK((ack.bitmap & (1u << 0)) != 0);  // seq0 missing
    CHECK((ack.bitmap & (1u << 2)) != 0);  // seq2 missing
    CHECK((ack.bitmap & (1u << 1)) == 0);  // seq1 present
    CHECK((ack.bitmap & (1u << 3)) == 0);  // seq3 present
}
static void test_reset() {
    Receiver r;
    r.recv_data(0, Bytes{3});
    r.reset();
    CHECK(r.expected() == 0 && r.out_of_order_count() == 0);
}

// ---- byte-golden ----
static void test_byte_golden(const char* hb_path, const char* ack_path) {
    Bytes gold_hb = read_file(hb_path);
    Bytes gold_ack = read_file(ack_path);
    CHECK(gold_hb.size() == 13 && gold_ack.size() == 13);

    // Reproduce the golden params (session 0x80, hdr stream NONE, hdr seq 1).
    Bytes my_ack = acknack_frame(0x80, STREAM_NONE, 1, 1, 0x00, 0x00, 0x80);
    Bytes my_hb = heartbeat_frame(0x80, STREAM_NONE, 1, 1, 3, 0x80);
    CHECK(my_ack == gold_ack);
    CHECK(my_hb == gold_hb);

    auto ph = parse_heartbeat(gold_hb.data(), gold_hb.size());
    CHECK(ph && ph->first_unacked == 1 && ph->last_unacked == 3 && ph->stream == 0x80);
    auto pa = parse_acknack(gold_ack.data(), gold_ack.size());
    CHECK(pa && pa->first_unacked == 1 && pa->bitmap == 0 && pa->stream == 0x80);
}

// ---- producer latency micro-bench (decoupled enqueue vs inline write+encode) ----
static void bench_latency() {
    const int K = 20000;
    std::uint8_t payload[16] = {0};
    std::vector<long> enq, inl;
    enq.reserve(K);
    inl.reserve(K);

    // Decoupled: enqueue into a ring (drain hooks are no-ops here — we measure
    // only the producer's wait-free push).
    AsyncWriter w([](const std::vector<Bytes>&) {}, [](const Bytes&) {},
                  [](std::uint8_t*, std::size_t) { return -1; }, 65536);
    for (int i = 0; i < K; ++i) {
        auto t0 = std::chrono::steady_clock::now();
        w.enqueue(payload, sizeof(payload));
        auto t1 = std::chrono::steady_clock::now();
        enq.push_back(std::chrono::duration_cast<std::chrono::nanoseconds>(t1 - t0).count());
    }
    w.stop();  // responder-less: tear down without waiting for the window to drain

    // Inline: what a naive writer does ON the producer thread — build the
    // WRITE_DATA frame AND push it through the kernel with a real sendto. This
    // is the syscall the decoupled enqueue avoids. The datagrams go to a bound
    // sink socket on loopback (received-and-dropped), so the cost measured is
    // the send-side kernel transition, not delivery.
    int sink = ::socket(AF_INET, SOCK_DGRAM, 0);
    sockaddr_in sa;
    std::memset(&sa, 0, sizeof(sa));
    sa.sin_family = AF_INET;
    sa.sin_port = 0;  // ephemeral
    inet_pton(AF_INET, "127.0.0.1", &sa.sin_addr);
    ::bind(sink, reinterpret_cast<sockaddr*>(&sa), sizeof(sa));
    socklen_t sl = sizeof(sa);
    ::getsockname(sink, reinterpret_cast<sockaddr*>(&sa), &sl);  // learn the port
    int tx = ::socket(AF_INET, SOCK_DGRAM, 0);
    ::connect(tx, reinterpret_cast<sockaddr*>(&sa), sizeof(sa));
    for (int i = 0; i < K; ++i) {
        auto t0 = std::chrono::steady_clock::now();
        Bytes f = write_frame(static_cast<std::uint16_t>(i), payload, sizeof(payload));
        ::send(tx, f.data(), f.size(), 0);
        auto t1 = std::chrono::steady_clock::now();
        inl.push_back(std::chrono::duration_cast<std::chrono::nanoseconds>(t1 - t0).count());
    }
    ::close(tx);
    ::close(sink);
    std::sort(enq.begin(), enq.end());
    std::sort(inl.begin(), inl.end());
    std::printf("LATENCY_NS enqueue_median=%ld inline_sendto_median=%ld\n",
                enq[K / 2], inl[K / 2]);
}

int main(int argc, char** argv) {
    test_submit_monotonic();
    test_submit_payload_too_large();
    test_submit_window_full();
    test_heartbeat();
    test_recv_acknack_clears();
    test_recv_acknack_full_clear();
    test_recv_in_order();
    test_recv_reorder();
    test_recv_duplicate_drop();
    test_recv_buffer_full();
    test_pending_acknack_bitmap();
    test_reset();
    if (argc >= 3) test_byte_golden(argv[1], argv[2]);
    else { std::fprintf(stderr, "no golden paths given\n"); ++g_fail; }
    bench_latency();

    if (g_fail == 0) {
        std::printf("ALL OK\n");
        return 0;
    }
    std::fprintf(stderr, "%d checks failed\n", g_fail);
    return 1;
}
