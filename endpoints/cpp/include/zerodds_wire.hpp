// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds_wire.hpp -- C++98 endpoint wire-core (ADR 0013).
//
// An idiomatic C++98 facade over the proven C89 wire-core (endpoints/c). The
// C++ SDK does NOT re-implement the wire -- it wraps the same byte-identical,
// host-endian-safe primitives, so it cannot drift from the Rust core. No
// C++11+, no Boost, minimal STL (std::string only).

#ifndef ZERODDS_WIRE_HPP
#define ZERODDS_WIRE_HPP

extern "C" {
#include "zerodds_wire.h"
}

#include <string>

namespace zerodds {

class Writer {
public:
    Writer(unsigned char* buf, size_t cap, int endian) {
        zdw_writer_init(&w_, buf, cap, endian);
    }
    int u8(unsigned char v)                 { return zdw_put_u8(&w_, v); }
    int u16(unsigned int v)                 { return zdw_put_u16(&w_, v); }
    int u32(unsigned long v)                { return zdw_put_u32(&w_, v); }
    int u64(zdw_u64_t v)                    { return zdw_put_u64(&w_, v); }
    int boolean(bool v)                     { return zdw_put_bool(&w_, v ? 1 : 0); }
    int f32(float v)                        { return zdw_put_f32(&w_, v); }
    int f64(double v)                       { return zdw_put_f64(&w_, v); }
    int str(const std::string& s)           { return zdw_put_string(&w_, s.c_str()); }
    int seq_u8(const unsigned char* d, size_t n) { return zdw_put_seq_u8(&w_, d, n); }

    // DHEADER (delimited CDR2): begin reserves the slot + returns the body
    // start; end back-patches it with the body byte length.
    size_t dheader_begin()                  { return zdw_dheader_begin(&w_); }
    int dheader_end(size_t body_start)      { return zdw_dheader_end(&w_, body_start); }

    // EMHEADER (@mutable, LC4): begin writes the EMHEADER + reserves the
    // NEXTINT; end back-patches it with the member body length.
    size_t emheader_begin(unsigned long member_id, bool must_understand) {
        return zdw_emheader_begin(&w_, member_id, must_understand ? 1 : 0);
    }
    int emheader_end(size_t body_start)     { return zdw_emheader_end(&w_, body_start); }

    size_t size() const                     { return w_.len; }
    int error() const                       { return w_.error; }

private:
    zdw_writer w_;
};

class Reader {
public:
    Reader(const unsigned char* buf, size_t len, int endian) {
        zdw_reader_init(&r_, buf, len, endian);
    }
    unsigned char u8()  { unsigned char v = 0; zdw_get_u8(&r_, &v); return v; }
    unsigned int  u16() { unsigned int v = 0; zdw_get_u16(&r_, &v); return v; }
    unsigned long u32() { unsigned long v = 0; zdw_get_u32(&r_, &v); return v; }
    zdw_u64_t     u64() { zdw_u64_t v; v.hi = 0; v.lo = 0; zdw_get_u64(&r_, &v); return v; }
    bool          boolean() { int v = 0; zdw_get_bool(&r_, &v); return v != 0; }
    float         f32() { float v = 0; zdw_get_f32(&r_, &v); return v; }
    double        f64() { double v = 0; zdw_get_f64(&r_, &v); return v; }
    std::string   str() {
        char buf[256];
        buf[0] = '\0';
        zdw_get_string(&r_, buf, sizeof(buf));
        return std::string(buf);
    }
    size_t seq_u8(unsigned char* out, size_t cap) {
        size_t n = 0;
        zdw_get_seq_u8(&r_, out, cap, &n);
        return n;
    }
    unsigned long dheader() { unsigned long v = 0; zdw_dheader_read(&r_, &v); return v; }
    // Reads an LC4 member header; returns the member id, sets *nextint.
    unsigned long emheader(unsigned long* nextint) {
        unsigned long id = 0; int mu = 0; unsigned long ni = 0;
        zdw_emheader_read(&r_, &id, &mu, &ni);
        if (nextint != 0) *nextint = ni;
        return id;
    }
    int error() const { return r_.error; }

private:
    zdw_reader r_;
};

} // namespace zerodds

#endif // ZERODDS_WIRE_HPP
