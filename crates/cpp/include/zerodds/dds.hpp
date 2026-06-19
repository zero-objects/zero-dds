// zerodds/dds.hpp — C++ RAII-wrapper convenience API over the C-FFI
// (zerodds.h). The spec-compliant DDS-PSM-Cxx 1.0 API lives under
// `dds/dds.hpp` (see `include/dds/`); this header here is the
// fast convenience path for Apex.AI plugins, ROS-2 RMW and
// embedded C++ apps that prefer a minimal `zerodds::Runtime/Writer/Reader`.
//
//   * `zerodds::Runtime`     — domain lifecycle (RAII).
//   * `zerodds::Writer`      — pub side, write(bytes).
//   * `zerodds::Reader`      — sub side, take() -> std::vector<uint8_t>.
//
// Memory: all classes are move-only, RAII destroys via zerodds_*_destroy.
// Buffer ownership on `take()`: fully automatic via vector copy.

#ifndef ZERODDS_DDS_HPP
#define ZERODDS_DDS_HPP

#include <cstdint>
#include <cstring>
#include <stdexcept>
#include <string>
#include <vector>

#include "zerodds.h"

namespace zerodds {

/// Status-code wrapper.
class StatusError : public std::runtime_error {
public:
    explicit StatusError(int code, const char *what)
        : std::runtime_error(std::string(what) + " (status=" + std::to_string(code) + ")"),
          code_(code) {}
    int code() const noexcept { return code_; }
private:
    int code_;
};

/// Domain runtime — wraps the C-FFI runtime handle.
class Runtime {
public:
    explicit Runtime(uint32_t domain_id)
        : handle_(zerodds_runtime_create(domain_id)) {
        if (!handle_) {
            throw StatusError(-1, "zerodds_runtime_create");
        }
    }
    ~Runtime() {
        if (handle_) zerodds_runtime_destroy(handle_);
    }

    Runtime(const Runtime &) = delete;
    Runtime &operator=(const Runtime &) = delete;
    Runtime(Runtime &&o) noexcept : handle_(o.handle_) { o.handle_ = nullptr; }
    Runtime &operator=(Runtime &&o) noexcept {
        if (this != &o) {
            if (handle_) zerodds_runtime_destroy(handle_);
            handle_ = o.handle_;
            o.handle_ = nullptr;
        }
        return *this;
    }

    /// Raw C handle — for direct FFI calls from friend classes.
    zerodds_ZeroDdsRuntime *raw() const noexcept { return handle_; }

private:
    zerodds_ZeroDdsRuntime *handle_;
};

/// DataWriter — pub-side, write(bytes+len).
class Writer {
public:
    Writer(Runtime &rt, const std::string &topic_name,
           const std::string &type_name, bool reliable = true)
        : handle_(zerodds_writer_create(rt.raw(), topic_name.c_str(),
                                         type_name.c_str(), reliable ? 1 : 0)) {
        if (!handle_) {
            throw StatusError(-1, "zerodds_writer_create");
        }
    }
    ~Writer() {
        if (handle_) zerodds_writer_destroy(handle_);
    }
    Writer(const Writer &) = delete;
    Writer &operator=(const Writer &) = delete;
    Writer(Writer &&o) noexcept : handle_(o.handle_) { o.handle_ = nullptr; }
    Writer &operator=(Writer &&o) noexcept {
        if (this != &o) {
            if (handle_) zerodds_writer_destroy(handle_);
            handle_ = o.handle_;
            o.handle_ = nullptr;
        }
        return *this;
    }

    /// Writes a sample. `data` points to already-CDR-encoded bytes.
    /// Throws `StatusError` on error.
    void write(const uint8_t *data, std::size_t len) {
        int rc = zerodds_writer_write(handle_, data, len);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_write");
    }
    void write(const std::vector<uint8_t> &payload) {
        write(payload.data(), payload.size());
    }

    /// Waits until `min_count` subscribers have matched or timeout.
    /// `true` = matched, `false` = timeout.
    bool wait_for_matched(int min_count, uint64_t timeout_ms) {
        int rc = zerodds_writer_wait_for_matched(handle_, min_count, timeout_ms);
        return rc == 0;
    }

    /// Spec §2.2.2.4.2.10 `dispose`. Sends a wire lifecycle
    /// marker — readers see the instance as `NotAliveDisposed`.
    /// `key_hash` is the 16-byte PLAIN_CDR2-BE key hash.
    void dispose(const uint8_t key_hash[16]) {
        int rc = zerodds_writer_dispose(handle_, key_hash);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_dispose");
    }

    /// Spec §2.2.2.4.2.7 `unregister_instance`.
    void unregister_instance(const uint8_t key_hash[16]) {
        int rc = zerodds_writer_unregister(handle_, key_hash);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_unregister");
    }

    /// Spec §2.2.3.21 with `autodispose=true` — combined marker.
    void unregister_with_dispose(const uint8_t key_hash[16]) {
        int rc = zerodds_writer_unregister_with_dispose(handle_, key_hash);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_unregister_with_dispose");
    }

private:
    zerodds_ZeroDdsWriter *handle_;
};

/// DataReader — sub-side, take() -> std::vector<uint8_t>.
class Reader {
public:
    Reader(Runtime &rt, const std::string &topic_name,
           const std::string &type_name, bool reliable = true)
        : handle_(zerodds_reader_create(rt.raw(), topic_name.c_str(),
                                         type_name.c_str(), reliable ? 1 : 0)) {
        if (!handle_) {
            throw StatusError(-1, "zerodds_reader_create");
        }
    }
    ~Reader() {
        if (handle_) zerodds_reader_destroy(handle_);
    }
    Reader(const Reader &) = delete;
    Reader &operator=(const Reader &) = delete;
    Reader(Reader &&o) noexcept : handle_(o.handle_) { o.handle_ = nullptr; }
    Reader &operator=(Reader &&o) noexcept {
        if (this != &o) {
            if (handle_) zerodds_reader_destroy(handle_);
            handle_ = o.handle_;
            o.handle_ = nullptr;
        }
        return *this;
    }

    /// Attempts to read a sample. Returns `std::vector<uint8_t>`
    /// — empty vector if nothing is available. Buffer-ownership transfer: the
    /// FFI allocates the raw buffer, we copy it into the vector and
    /// free the raw one via `zerodds_buffer_free`.
    std::vector<uint8_t> take() {
        uint8_t *raw = nullptr;
        std::size_t len = 0;
        int rc = zerodds_reader_take(handle_, &raw, &len);
        if (rc != 0) throw StatusError(rc, "zerodds_reader_take");
        if (!raw || len == 0) return {};
        std::vector<uint8_t> out(raw, raw + len);
        zerodds_buffer_free(raw, len);
        return out;
    }

    /// Waits until `min_count` publishers have matched.
    bool wait_for_matched(int min_count, uint64_t timeout_ms) {
        int rc = zerodds_reader_wait_for_matched(handle_, min_count, timeout_ms);
        return rc == 0;
    }

private:
    zerodds_ZeroDdsReader *handle_;
};

/// Version string — returns the C-FFI package version.
inline const char *version() {
    return zerodds_version();
}

}  // namespace zerodds

#endif  // ZERODDS_DDS_HPP
