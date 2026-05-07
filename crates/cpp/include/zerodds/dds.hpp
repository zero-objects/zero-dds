// zerodds/dds.hpp — C++ RAII-Wrapper-Convenience-API ueber das C-FFI
// (zerodds.h). Die spec-konforme DDS-PSM-Cxx 1.0 API lebt unter
// `dds/dds.hpp` (siehe `include/dds/`); dieser Header hier ist der
// schnelle Convenience-Pfad fuer Apex.AI-Plugins, ROS-2-RMW und
// embedded-C++-Apps, die ein minimales `zerodds::Runtime/Writer/Reader`
// bevorzugen.
//
//   * `zerodds::Runtime`     — Domain-Lifecycle (RAII).
//   * `zerodds::Writer`      — Pub-Side, write(bytes).
//   * `zerodds::Reader`      — Sub-Side, take() -> std::vector<uint8_t>.
//
// Memory: alle Klassen sind move-only, RAII destroys via zerodds_*_destroy.
// Buffer-Ownership bei `take()`: vollautomatisch via Vector-Kopie.

#ifndef ZERODDS_DDS_HPP
#define ZERODDS_DDS_HPP

#include <cstdint>
#include <cstring>
#include <stdexcept>
#include <string>
#include <vector>

#include "zerodds.h"

namespace zerodds {

/// Statuscode-Wrapper.
class StatusError : public std::runtime_error {
public:
    explicit StatusError(int code, const char *what)
        : std::runtime_error(std::string(what) + " (status=" + std::to_string(code) + ")"),
          code_(code) {}
    int code() const noexcept { return code_; }
private:
    int code_;
};

/// Domain-Runtime — wrap des C-FFI Runtime-Handles.
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

    /// Roher C-Handle — fuer direct-FFI-Aufrufe von Friend-Klassen.
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

    /// Schreibt einen Sample. `data` zeigt auf bereits-CDR-encodete Bytes.
    /// Wirft `StatusError` bei Fehler.
    void write(const uint8_t *data, std::size_t len) {
        int rc = zerodds_writer_write(handle_, data, len);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_write");
    }
    void write(const std::vector<uint8_t> &payload) {
        write(payload.data(), payload.size());
    }

    /// Wartet bis `min_count` Subscribers gematcht haben oder Timeout.
    /// `true` = matched, `false` = timeout.
    bool wait_for_matched(int min_count, uint64_t timeout_ms) {
        int rc = zerodds_writer_wait_for_matched(handle_, min_count, timeout_ms);
        return rc == 0;
    }

    /// Spec §2.2.2.4.2.10 `dispose`. Schickt einen Wire-Lifecycle-
    /// Marker — Reader sehen die Instanz als `NotAliveDisposed`.
    /// `key_hash` ist der 16-byte PLAIN_CDR2-BE-Schluesselhash.
    void dispose(const uint8_t key_hash[16]) {
        int rc = zerodds_writer_dispose(handle_, key_hash);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_dispose");
    }

    /// Spec §2.2.2.4.2.7 `unregister_instance`.
    void unregister_instance(const uint8_t key_hash[16]) {
        int rc = zerodds_writer_unregister(handle_, key_hash);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_unregister");
    }

    /// Spec §2.2.3.21 mit `autodispose=true` — kombinierter Marker.
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

    /// Versucht einen Sample zu lesen. Returnt `std::vector<uint8_t>`
    /// — leerer Vector wenn nichts da. Buffer-Ownership-Wechsel: das
    /// FFI alloziert den raw buffer, wir kopieren ihn in den Vector und
    /// freigeben den raw via `zerodds_buffer_free`.
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

    /// Wartet bis `min_count` Publishers gematcht haben.
    bool wait_for_matched(int min_count, uint64_t timeout_ms) {
        int rc = zerodds_reader_wait_for_matched(handle_, min_count, timeout_ms);
        return rc == 0;
    }

private:
    zerodds_ZeroDdsReader *handle_;
};

/// Version-String — liefert den C-FFI-PKG-Version.
inline const char *version() {
    return zerodds_version();
}

}  // namespace zerodds

#endif  // ZERODDS_DDS_HPP
