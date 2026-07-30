// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Async reactor test (C++17): an AsyncWriter fires N samples into an in-memory
// FIFO transport; the AsyncReader drains them via a std::function callback that
// decodes each. Proves the modern facade dispatches every sample.

#include <cstdio>
#include <cstring>
#include <vector>

#include "zerodds_async.hpp"
#include "zerodds_wire.hpp"

namespace {

constexpr int kSlots = 16;
constexpr int kFrame = 256;
constexpr int kN = 5;

struct Fifo {
    unsigned char buf[kSlots][kFrame];
    std::size_t   len[kSlots];
    int head = 0, tail = 0, count = 0;
};

extern "C" int fifo_deliver(void* ctx, const unsigned char* frame, std::size_t len) {
    auto* f = static_cast<Fifo*>(ctx);
    if (f->count == kSlots || len > kFrame) return ZDW_T_ERROR;
    std::memcpy(f->buf[f->tail], frame, len);
    f->len[f->tail] = len;
    f->tail = (f->tail + 1) % kSlots;
    f->count++;
    return ZDW_T_OK;
}

extern "C" int fifo_receive(void* ctx, unsigned char* out, std::size_t cap, std::size_t* len) {
    auto* f = static_cast<Fifo*>(ctx);
    if (f->count == 0) return ZDW_T_AGAIN;
    if (f->len[f->head] > cap) return ZDW_T_ERROR;
    std::memcpy(out, f->buf[f->head], f->len[f->head]);
    *len = f->len[f->head];
    f->head = (f->head + 1) % kSlots;
    f->count--;
    return ZDW_T_OK;
}

// Minimal sensor codec via the C++98 wire facade (id + a fixed tail).
std::size_t encode_sample(unsigned char* out, std::size_t cap, unsigned long id) {
    zerodds::Writer w(out, cap, ZDW_LE);
    w.u32(id);
    w.u16(0);
    w.u8(0);
    w.f32(0.0f);
    w.u64(zdw_u64_from_ul(0));
    w.str("cpp");
    w.seq_u8(nullptr, 0);
    return w.size();
}

unsigned long decode_id(const unsigned char* body, std::size_t len) {
    zerodds::Reader r(body, len, ZDW_LE);
    return r.u32();
}

}  // namespace

int main() {
    Fifo fq{};
    zdw_transport t{&fq, fifo_deliver, fifo_receive};

    std::vector<unsigned long> got;
    unsigned char rxbuf[kFrame];
    zerodds::AsyncReader reader(&t, rxbuf, sizeof rxbuf,
        [&got](const unsigned char* body, std::size_t len) {
            got.push_back(decode_id(body, len));
        });

    unsigned char txbuf[kFrame];
    zerodds::AsyncWriter writer(&t, txbuf, sizeof txbuf,
                                ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT);

    for (int i = 0; i < kN; i++) {
        unsigned char body[128];
        std::size_t n = encode_sample(body, sizeof body, 0x1000u + i);
        if (!writer.write(body, n)) {
            std::fprintf(stderr, "write %d failed\n", i);
            return 1;
        }
    }

    int dispatched = reader.run();
    if (dispatched != kN || static_cast<int>(got.size()) != kN) {
        std::fprintf(stderr, "dispatched %d, got %zu, expected %d\n",
                     dispatched, got.size(), kN);
        return 1;
    }
    for (int i = 0; i < kN; i++) {
        if (got[i] != 0x1000u + static_cast<unsigned long>(i)) {
            std::fprintf(stderr, "sample %d: id 0x%lX out of order\n", i, got[i]);
            return 1;
        }
    }

    std::printf("async loopback (C++17): %d samples dispatched + decoded in order\n", kN);
    std::printf("ALL OK\n");
    return 0;
}
