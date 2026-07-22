// SPDX-License-Identifier: Apache-2.0
// C++98 reflective codec reuses the C zerodds_reflect directly (ADR 0013).
// Builds the @mutable MutableM via the C recursive struct API + compares.
// usage: test_reflect <golden_mutable_le.bin> <golden_mutable_be.bin>
#include <cstdio>
#include <cstring>
extern "C" {
#include "zerodds_reflect.h"
}
static long rd(const char* p, unsigned char* b, long c) {
    std::FILE* f = std::fopen(p, "rb"); if (!f) return -1;
    long n = (long)std::fread(b, 1, (size_t)c, f); std::fclose(f); return n;
}
static int check(int endian, const char* gp, const char* tag) {
    zdw_dyn_field f[3]; unsigned long ids[3]; zdw_dyn_struct s;
    unsigned char out[256], g[256]; long gn; zdw_writer w;
    std::memset(f, 0, sizeof(f));
    ids[0]=10; ids[1]=20; ids[2]=30;
    f[0].kind=ZDW_K_U32; f[0].u32=0xDEADBEEFuL;
    f[1].kind=ZDW_K_STRING; f[1].str=(char*)"mut";
    f[2].kind=ZDW_K_U16; f[2].u16=0x0777u;
    s.ext=ZDW_X_MUTABLE; s.fields=f; s.ids=ids; s.n=3;
    zdw_writer_init(&w, out, sizeof(out), endian);
    if (zdw_reflect_encode_struct(&w, &s) != ZDW_OK) { std::fprintf(stderr, "%s enc\n", tag); return 1; }
    gn = rd(gp, g, (long)sizeof(g));
    if ((long)w.len != gn || std::memcmp(out, g, (size_t)gn) != 0) { std::fprintf(stderr, "%s mismatch\n", tag); return 1; }
    std::printf("%s: %ld bytes byte-identical (C++ reflective @mutable via C)\n", tag, (long)w.len);
    return 0;
}
int main(int argc, char** argv) {
    if (argc < 3) { std::fprintf(stderr, "usage: %s le be\n", argv[0]); return 2; }
    int rc = check(ZDW_LE, argv[1], "LE") | check(ZDW_BE, argv[2], "BE");
    if (rc == 0) std::printf("ALL OK\n");
    return rc;
}
