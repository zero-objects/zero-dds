// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds_async.hpp -- modern C++17 async facade for the native endpoint SDK
// (ADR 0013). ADDITIVE: the conservative C++98 wire facade (zerodds_wire.hpp)
// stays untouched; this is the modern-toolchain option (std::function
// callbacks, RAII, std::optional) for teams that want event-driven,
// non-blocking receive.
//
// Like the rest of the C++ SDK it is a thin facade over the audited C reactor
// (endpoints/c zerodds_async.c) -- it does NOT re-implement the wire, so it
// cannot drift from the byte-identical core.

#ifndef ZERODDS_ASYNC_HPP
#define ZERODDS_ASYNC_HPP

extern "C" {
#include "zerodds_async.h"
}

#include <cstddef>
#include <functional>
#include <utility>

namespace zerodds {

// An event-driven reader: dispatches each received sample body to a
// std::function callback via the transport's non-blocking receive.
class AsyncReader {
public:
    using OnSample = std::function<void(const unsigned char* body, std::size_t len)>;

    AsyncReader(const zdw_transport* transport, unsigned char* rxbuf,
                std::size_t rxcap, OnSample on_sample)
        : on_sample_(std::move(on_sample)) {
        zdw_async_reader_init(&r_, transport, rxbuf, rxcap, &AsyncReader::trampoline, this);
    }

    AsyncReader(const AsyncReader&) = delete;
    AsyncReader& operator=(const AsyncReader&) = delete;

    // Dispatch one ready frame. Returns ZDW_T_OK / ZDW_T_AGAIN / ZDW_T_ERROR.
    int poll() { return zdw_async_poll(&r_); }

    // Drain the transport (until ZDW_T_AGAIN, or `max` frames). Returns count.
    int run(int max = 0) { return zdw_async_run(&r_, max); }

private:
    static void trampoline(void* ctx, const unsigned char* body, std::size_t len) {
        static_cast<AsyncReader*>(ctx)->on_sample_(body, len);
    }
    zdw_async_reader r_{};
    OnSample on_sample_;
};

// A fire-and-forget writer: frames an XCDR sample as XRCE WRITE_DATA.
class AsyncWriter {
public:
    AsyncWriter(const zdw_transport* transport, unsigned char* txbuf,
                std::size_t txcap, unsigned char session, unsigned char stream) {
        zdw_async_writer_init(&w_, transport, txbuf, txcap, session, stream);
    }

    // Returns true on delivery.
    bool write(const unsigned char* sample, std::size_t len) {
        return zdw_async_write(&w_, sample, len) == ZDW_T_OK;
    }

private:
    zdw_async_writer w_{};
};

}  // namespace zerodds

#endif  // ZERODDS_ASYNC_HPP
