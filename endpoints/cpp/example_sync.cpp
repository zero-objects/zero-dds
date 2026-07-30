// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deep example (sync, C++98 facade): a realistic sensor-telemetry flow. A
// publisher frames five typed Reading{ id, value, label } samples and delivers
// them; the subscriber owns the run-loop and polls, decoding EVERY field
// byte-for-byte via the conservative C++ wire facade.

#include <cstdio>
#include <cstring>
#include <string>

#include "zerodds_endpoint.h"
#include "zerodds_wire.hpp"

namespace {

const int kSlots = 16;
const int kFrame = 256;
const int kTotal = 5;

// An integrator-supplied transport: a small FIFO of frames.
struct Fifo {
    unsigned char buf[kSlots][kFrame];
    std::size_t   len[kSlots];
    int head, tail, count;
};

extern "C" int fifo_deliver(void* ctx, const unsigned char* frame, std::size_t len) {
    Fifo* f = static_cast<Fifo*>(ctx);
    if (f->count == kSlots || len > kFrame) return ZDW_T_ERROR;
    std::memcpy(f->buf[f->tail], frame, len);
    f->len[f->tail] = len;
    f->tail = (f->tail + 1) % kSlots;
    f->count++;
    return ZDW_T_OK;
}

extern "C" int fifo_receive(void* ctx, unsigned char* out, std::size_t cap, std::size_t* len) {
    Fifo* f = static_cast<Fifo*>(ctx);
    if (f->count == 0) return ZDW_T_AGAIN;
    if (f->len[f->head] > cap) return ZDW_T_ERROR;
    std::memcpy(out, f->buf[f->head], f->len[f->head]);
    *len = f->len[f->head];
    f->head = (f->head + 1) % kSlots;
    f->count--;
    return ZDW_T_OK;
}

// Reading { uint32 id; float value; string label }.
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
    Reading rd;
    rd.id = r.u32();
    rd.value = r.f32();
    rd.label = r.str();
    return rd;
}

}  // namespace

int main() {
    Fifo fq;
    std::memset(&fq, 0, sizeof(fq));
    zdw_transport t;
    t.ctx = &fq;
    t.deliver = fifo_deliver;
    t.receive = fifo_receive;

    for (int i = 0; i < kTotal; i++) {
        Reading rd;
        char lbl[32];
        unsigned char body[128];
        unsigned char frame[160];
        std::size_t body_len, frame_len;

        rd.id = 0x1000UL + static_cast<unsigned long>(i);
        rd.value = 20.0f + static_cast<float>(i) * 0.5f;
        std::sprintf(lbl, "bay-%02d", i);
        rd.label = lbl;

        body_len = marshal(rd, body, sizeof(body));
        frame_len = zdw_xrce_write_frame(frame, sizeof(frame),
            ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT,
            static_cast<unsigned int>(i + 1), body, body_len);
        if (frame_len == 0 || t.deliver(t.ctx, frame, frame_len) != ZDW_T_OK) {
            std::fprintf(stderr, "publish %d failed\n", i);
            return 1;
        }
    }

    int got = 0;
    for (;;) {
        unsigned char frame[256];
        std::size_t frame_len = 0;
        const unsigned char* body = 0;
        std::size_t body_len = 0;

        if (t.receive(t.ctx, frame, sizeof(frame), &frame_len) != ZDW_T_OK) break;
        if (zdw_xrce_read_frame(frame, frame_len, &body, &body_len) != ZDW_T_OK) continue;
        Reading rd = unmarshal(body, body_len);
        std::printf("sync reading %d: id=0x%lx value=%.1f label=\"%s\"\n",
                    got, rd.id, static_cast<double>(rd.value), rd.label.c_str());
        got++;
    }

    if (got != kTotal) {
        std::fprintf(stderr, "incomplete: got %d of %d\n", got, kTotal);
        return 1;
    }
    std::printf("ALL OK\n");
    return 0;
}
