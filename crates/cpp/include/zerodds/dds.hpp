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

// Typed convenience writers/readers (`zerodds::TypedWriter<T>` /
// `zerodds::TypedReader<T>`) are built on the same
// `dds::topic::topic_type_support<T>` trait that `idlc cpp` emits for
// every IDL struct — so an IDL-generated type drops straight in.
#include "../dds/topic/TopicTraits.hpp"
#include "../dds/topic/xcdr2.hpp"

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
        // `out_repr` (XCDR1/XCDR2) is discarded here — the byte-oriented
        // convenience Reader hands back the raw payload unchanged. Typed
        // consumers (TypedReader<T>) capture it for decode alignment.
        uint8_t repr = 0;
        int rc = zerodds_reader_take(handle_, &raw, &len, &repr);
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

/// Typed DataWriter — publishes an IDL-generated `T` directly.
///
/// `T` must have a `dds::topic::topic_type_support<T>` specialization
/// (emitted by `idlc cpp` for every struct, or hand-written). The
/// topic's type name defaults to `topic_type_support<T>::type_name()`,
/// the keyed/no-keyed flag to `topic_type_support<T>::is_keyed()`.
///
/// On `write(value)` the sample is XCDR2-encoded via the trait's
/// `encode()` and handed to the byte-oriented `zerodds_writer_write`.
template <typename T>
class TypedWriter {
    using traits = ::dds::topic::topic_type_support<T>;

public:
    /// 2-arg ctor: type name + keyed flag derived from the type traits.
    TypedWriter(Runtime &rt, const std::string &topic_name, bool reliable = true)
        : handle_(zerodds_writer_create_kind(
              rt.raw(), topic_name.c_str(), traits::type_name(),
              reliable ? 1 : 0, traits::is_keyed() ? 1 : 0)) {
        if (!handle_) {
            throw StatusError(-1, "zerodds_writer_create_kind");
        }
    }
    ~TypedWriter() {
        if (handle_) zerodds_writer_destroy(handle_);
    }
    TypedWriter(const TypedWriter &) = delete;
    TypedWriter &operator=(const TypedWriter &) = delete;
    TypedWriter(TypedWriter &&o) noexcept : handle_(o.handle_) { o.handle_ = nullptr; }
    TypedWriter &operator=(TypedWriter &&o) noexcept {
        if (this != &o) {
            if (handle_) zerodds_writer_destroy(handle_);
            handle_ = o.handle_;
            o.handle_ = nullptr;
        }
        return *this;
    }

    /// Publishes a typed sample (XCDR2 wire). Throws on error.
    void write(const T &value) {
        std::vector<uint8_t> bytes = traits::encode(value);
        int rc = zerodds_writer_write(handle_, bytes.data(), bytes.size());
        if (rc != 0) throw StatusError(rc, "zerodds_writer_write");
    }

    /// Waits until `min_count` subscribers have matched or timeout.
    bool wait_for_matched(int min_count, uint64_t timeout_ms) {
        int rc = zerodds_writer_wait_for_matched(handle_, min_count, timeout_ms);
        return rc == 0;
    }

private:
    zerodds_ZeroDdsWriter *handle_;
};

/// Typed DataReader — reads an IDL-generated `T` directly.
///
/// Mirror of `TypedWriter<T>`. `take()` returns the decoded `T` (or an
/// empty optional-like result via the bool overload) — the wire bytes
/// are XCDR2/XCDR1-decoded with the representation reported by the
/// C-FFI `out_repr` out-param, so alignment matches the publisher.
template <typename T>
class TypedReader {
    using traits = ::dds::topic::topic_type_support<T>;

public:
    TypedReader(Runtime &rt, const std::string &topic_name, bool reliable = true)
        : handle_(zerodds_reader_create_kind(
              rt.raw(), topic_name.c_str(), traits::type_name(),
              reliable ? 1 : 0, traits::is_keyed() ? 1 : 0)) {
        if (!handle_) {
            throw StatusError(-1, "zerodds_reader_create_kind");
        }
    }
    ~TypedReader() {
        if (handle_) zerodds_reader_destroy(handle_);
    }
    TypedReader(const TypedReader &) = delete;
    TypedReader &operator=(const TypedReader &) = delete;
    TypedReader(TypedReader &&o) noexcept : handle_(o.handle_) { o.handle_ = nullptr; }
    TypedReader &operator=(TypedReader &&o) noexcept {
        if (this != &o) {
            if (handle_) zerodds_reader_destroy(handle_);
            handle_ = o.handle_;
            o.handle_ = nullptr;
        }
        return *this;
    }

    /// Attempts to read+decode a sample. Returns `true` and fills
    /// `out` if one was available, `false` otherwise.
    bool take(T &out) {
        uint8_t *raw = nullptr;
        std::size_t len = 0;
        uint8_t repr = 0;
        int rc = zerodds_reader_take(handle_, &raw, &len, &repr);
        if (rc != 0) throw StatusError(rc, "zerodds_reader_take");
        if (!raw || len == 0) return false;
        auto version = repr == 1 ? ::dds::topic::xcdr2::XcdrVersion::Xcdr2
                                 : ::dds::topic::xcdr2::XcdrVersion::Xcdr1;
        try {
            out = traits::decode(raw, len, version);
        } catch (...) {
            zerodds_buffer_free(raw, len);
            throw;
        }
        zerodds_buffer_free(raw, len);
        return true;
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
