// SPDX-License-Identifier: Apache-2.0
// C++98 endpoints reuse the C XRCE/serial framing directly (ADR 0013).
// usage: test_endpoint <golden_xrce_le.bin> <golden_serial_le.bin>
#include <cstdio>
#include <cstring>
extern "C" {
#include "zerodds_endpoint.h"
}
#include "zerodds_wire.hpp"

static long rd(const char* p, unsigned char* b, long c) {
    std::FILE* f = std::fopen(p, "rb"); if (!f) return -1;
    long n = (long)std::fread(b, 1, (size_t)c, f); std::fclose(f); return n;
}
int main(int argc, char** argv) {
    if (argc < 3) { std::fprintf(stderr, "usage: %s xrce serial\n", argv[0]); return 2; }
    unsigned char body[256], frame[512], sframe[600], gx[512], gs[600];
    zerodds::Writer w(body, sizeof(body), ZDW_LE);
    zdw_u64_t st; st.hi = 0x01020304uL; st.lo = 0x05060708uL;
    unsigned char raw[4]; raw[0]=0xDE; raw[1]=0xAD; raw[2]=0xBE; raw[3]=0xEF;
    w.u32(0xA1B2C3D4uL); w.u16(0x1234u); w.u8(0x5Au); w.f32(3.5f); w.u64(st);
    w.str("bay-12"); w.seq_u8(raw, 4);
    size_t flen = zdw_xrce_write_frame(frame, sizeof(frame), ZDW_XRCE_SESSION_NOKEY,
                                       ZDW_XRCE_STREAM_BEST_EFFORT, 1, body, w.size());
    size_t slen = zdw_serial_frame(sframe, sizeof(sframe), frame, flen);
    long gxn = rd(argv[1], gx, (long)sizeof(gx));
    long gsn = rd(argv[2], gs, (long)sizeof(gs));
    if ((long)flen != gxn || std::memcmp(frame, gx, (size_t)gxn) != 0) { std::printf("XRCE mismatch\n"); return 1; }
    if ((long)slen != gsn || std::memcmp(sframe, gs, (size_t)gsn) != 0) { std::printf("serial mismatch\n"); return 1; }
    std::printf("C++ XRCE %ld + serial %ld byte-identical to crates/xrce\nALL OK\n", (long)flen, (long)slen);
    return 0;
}
