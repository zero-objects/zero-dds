// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runnable reliable-stream demo (in-process, no network): an aggregator submits
// N samples, a lossy channel drops some, and the reliable receiver recovers
// every sample in order via ACKNACK-driven retransmit. Prints the contiguous
// delivered sequence. Build: g++ -std=c++17 -Iinclude example_reliable.cpp

#include "zerodds_reliable.hpp"

#include <cstdint>
#include <cstdio>
#include <vector>

using namespace zerodds::reliable;

static Bytes le32(std::uint32_t v) {
    return Bytes{static_cast<std::uint8_t>(v), static_cast<std::uint8_t>(v >> 8),
                 static_cast<std::uint8_t>(v >> 16), static_cast<std::uint8_t>(v >> 24)};
}

int main() {
    const std::uint32_t N = 16;
    Sender snd;
    Receiver rcv;

    // Aggregator submits N samples (payload = LE index).
    for (std::uint32_t i = 0; i < N; ++i) {
        std::uint16_t seq = 0;
        snd.submit(le32(i), seq);
    }

    std::vector<std::uint32_t> delivered;
    auto deliver = [&](const std::vector<std::pair<std::uint16_t, Bytes>>& v) {
        for (const auto& p : v) {
            std::uint32_t x = p.second[0] | (p.second[1] << 8) | (p.second[2] << 16) |
                              (static_cast<std::uint32_t>(p.second[3]) << 24);
            delivered.push_back(x);
        }
    };

    // Round 0: send every in-flight frame; the lossy channel drops every 3rd.
    int idx = 0;
    for (const auto& kv : snd.in_flight()) {
        ++idx;
        if (idx % 3 == 0) continue;  // dropped in flight
        rcv.recv_data(kv.first, kv.second);
    }
    deliver(rcv.drain_in_order());

    // Recovery rounds: receiver NACKs the gaps, sender retransmits (no loss now).
    int guard = 0;
    while (rcv.expected() < N && guard++ < 100) {
        AckNack ack = rcv.pending_acknack(std::nullopt);
        std::uint16_t base = static_cast<std::uint16_t>(ack.first_unacked);
        snd.recv_acknack(base, ack.bitmap);
        for (std::uint16_t i = 0; i < 16; ++i) {
            if (((ack.bitmap >> i) & 1u) == 0) continue;
            std::uint16_t seq = static_cast<std::uint16_t>(base + i);
            if (const Bytes* p = snd.get_in_flight(seq)) rcv.recv_data(seq, *p);
        }
        deliver(rcv.drain_in_order());
    }

    std::printf("delivered:");
    for (std::uint32_t x : delivered) std::printf(" %u", x);
    std::printf("\n");

    bool contiguous = delivered.size() == N;
    for (std::uint32_t i = 0; i < delivered.size(); ++i)
        if (delivered[i] != i) contiguous = false;
    std::printf(contiguous ? "RELIABLE OK N=%u\n" : "RELIABLE FAIL\n", N);
    return contiguous ? 0 : 1;
}
