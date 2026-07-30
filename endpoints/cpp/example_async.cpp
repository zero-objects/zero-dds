// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deep example (async, C++17 facade): the same sensor-telemetry flow, but the
// subscriber does not own the run-loop. An AsyncWriter fires five typed Reading
// samples into an in-memory FIFO; the AsyncReader drains them via a
// std::function callback that decodes EVERY field — the modern-toolchain,
// event-driven facade.

#include <cstdio>
#include <cstring>
#include <string>

#include "zerodds_async.hpp"
#include "zerodds_wire.hpp"

namespace {

constexpr int kSlots = 16;
constexpr int kFrame = 256;
constexpr int kTotal = 5;

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

struct Reading {
    unsigned long id;
    float         value;
    std::string   label;
};

std::size_t marshal(const Reading& rd, unsigned char* out, std::size_t cap) {
    zerodds::Writer w(out, cap, ZDW_LE);
    w.u32(rd.id);
    w.f32(rd.value);
    w.str(rd.label);
    return w.size();
}

Reading unmarshal(const unsigned char* body, std::size_t len) {
    zerodds::Reader r(body, len, ZDW_LE);
    return Reading{r.u32(), r.f32(), r.str()};
}

}  // namespace

int main() {
    Fifo fq{};
    zdw_transport t{&fq, fifo_deliver, fifo_receive};

    unsigned char rxbuf[kFrame];
    int got = 0;
    zerodds::AsyncReader reader(&t, rxbuf, sizeof(rxbuf),
        [&got](const unsigned char* body, std::size_t len) {
            Reading rd = unmarshal(body, len);
            std::printf("async reading %d: id=0x%lx value=%.1f label=\"%s\"\n",
                        got, rd.id, static_cast<double>(rd.value), rd.label.c_str());
            got++;
        });

    unsigned char txbuf[kFrame];
    zerodds::AsyncWriter writer(&t, txbuf, sizeof(txbuf),
                                ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT);

    for (int i = 0; i < kTotal; i++) {
        char lbl[32];
        std::snprintf(lbl, sizeof(lbl), "sensor-%02d", i);
        Reading rd{0x2000UL + static_cast<unsigned long>(i),
                   100.0f - static_cast<float>(i), lbl};
        unsigned char body[128];
        std::size_t body_len = marshal(rd, body, sizeof(body));
        if (!writer.write(body, body_len)) {
            std::fprintf(stderr, "write %d failed\n", i);
            return 1;
        }
    }

    int dispatched = reader.run(0);
    if (dispatched != kTotal || got != kTotal) {
        std::fprintf(stderr, "dispatched %d, decoded %d, expected %d\n",
                     dispatched, got, kTotal);
        return 1;
    }
    std::printf("ALL OK\n");
    return 0;
}
