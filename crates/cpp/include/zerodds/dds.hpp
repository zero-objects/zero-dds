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


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <cstring>
#include <stdexcept>
#include <string>
#include <vector>

#include "zerodds.h"

// Typed convenience writers/readers (`zerodds::TypedWriter<T>` /
// `zerodds::TypedReader<T>`) are built on the same
// `dds::topic::topic_type_support<T>` trait that
// `zerodds-idlc generate --lang c++` emits for every IDL struct (with
// `--cpp-mapping native` the structures of the codec, with the default `psm` the classes of
// DDS-PSM-Cxx 1.0 §7.4 over them) — so an IDL-generated type drops straight in.
#include "../dds/topic/TopicTraits.hpp"
#include "zerodds/wire.hpp"

namespace zerodds {

/// Status-code wrapper.
class StatusError : public std::runtime_error {
public:
    explicit StatusError(int code, const char *what)
        : std::runtime_error(std::string(what) + " (status=" + ::zerodds::cxx::to_string(code) + ")"),
          code_(code) {}
    int code() const ZERODDS_NOEXCEPT { return code_; }
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

    ZERODDS_NONCOPYABLE(Runtime)
    Runtime(Runtime &&o) ZERODDS_NOEXCEPT : handle_(o.handle_) { o.handle_ = nullptr; }
    Runtime &operator=(Runtime &&o) ZERODDS_NOEXCEPT {
        if (this != &o) {
            if (handle_) zerodds_runtime_destroy(handle_);
            handle_ = o.handle_;
            o.handle_ = nullptr;
        }
        return *this;
    }

    /// Raw C handle — for direct FFI calls from friend classes.
    zerodds_ZeroDdsRuntime *raw() const ZERODDS_NOEXCEPT { return handle_; }

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
    ZERODDS_NONCOPYABLE(Writer)
    Writer(Writer &&o) ZERODDS_NOEXCEPT : handle_(o.handle_) { o.handle_ = nullptr; }
    Writer &operator=(Writer &&o) ZERODDS_NOEXCEPT {
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
    ///
    /// This form carries no instance value, only its hash: the DATA it
    /// sends carries `PID_KEY_HASH` alone, no `serializedKey`. RTI Connext
    /// and CoreDX accept that form; Eclipse Cyclone 11.0.1, Fast DDS 3.6.1
    /// and OpenDDS 3.35 do not treat it as a dispose
    /// (`Test_FinalInstanceState_1`, measured 0/4 on all three). Prefer
    /// `dispose(const uint8_t *, std::size_t)` below (or
    /// `TypedWriter<T>::dispose(const T &)`) when the instance's encoded
    /// sample is available — both derive the full `serializedKey` form
    /// all five vendors accept.
    void dispose(const uint8_t key_hash[16]) {
        int rc = zerodds_writer_dispose(handle_, key_hash);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_dispose");
    }

    /// Spec §2.2.2.4.2.10 `dispose`, derived from the sample itself
    /// instead of a caller-computed key hash. `sample`/`len` are this
    /// writer's own encoding of the instance, exactly as `write()` frames
    /// it. Requires a writer created via `zerodds_writer_create_typed`
    /// with a keyed type — throws `StatusError` (status
    /// `PreconditionNotMet`) otherwise, never a silent fallback to the
    /// hash-only form.
    void dispose(const uint8_t *sample, std::size_t len) {
        int rc = zerodds_writer_dispose_sample(handle_, sample, len);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_dispose_sample");
    }

    /// Spec §2.2.2.4.2.7 `unregister_instance`. Same `PID_KEY_HASH`-only
    /// caveat as [`dispose(const uint8_t[16])`] above — prefer
    /// `unregister_instance(const uint8_t *, std::size_t)` when the
    /// sample is available.
    void unregister_instance(const uint8_t key_hash[16]) {
        int rc = zerodds_writer_unregister(handle_, key_hash);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_unregister");
    }

    /// Spec §2.2.2.4.2.7 `unregister_instance`, derived from the sample —
    /// see `dispose(const uint8_t *, std::size_t)`.
    void unregister_instance(const uint8_t *sample, std::size_t len) {
        int rc = zerodds_writer_unregister_sample(handle_, sample, len);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_unregister_sample");
    }

    /// Spec §2.2.3.21 with `autodispose=true` — combined marker. Same
    /// `PID_KEY_HASH`-only caveat as [`dispose(const uint8_t[16])`] above
    /// — prefer `unregister_with_dispose(const uint8_t *, std::size_t)`
    /// when the sample is available.
    void unregister_with_dispose(const uint8_t key_hash[16]) {
        int rc = zerodds_writer_unregister_with_dispose(handle_, key_hash);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_unregister_with_dispose");
    }

    /// Spec §2.2.3.21 with `autodispose=true`, derived from the sample —
    /// see `dispose(const uint8_t *, std::size_t)`.
    void unregister_with_dispose(const uint8_t *sample, std::size_t len) {
        int rc = zerodds_writer_unregister_with_dispose_sample(handle_, sample, len);
        if (rc != 0) throw StatusError(rc, "zerodds_writer_unregister_with_dispose_sample");
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
    ZERODDS_NONCOPYABLE(Reader)
    Reader(Reader &&o) ZERODDS_NOEXCEPT : handle_(o.handle_) { o.handle_ = nullptr; }
    Reader &operator=(Reader &&o) ZERODDS_NOEXCEPT {
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
/// the keyed/no-keyed flag to `topic_type_support<T>::is_keyed()`, and the
/// encapsulation identifier the C ABI stamps to
/// `topic_type_support<T>::extensibility()`.
///
/// On `write(value)` the sample is XCDR2-encoded via the trait's
/// `encode()` and handed to the byte-oriented `zerodds_writer_write`.
template <typename T>
class TypedWriter {
    typedef ::dds::topic::topic_type_support<T> traits;

public:
    /// 2-arg ctor: type name + keyed flag derived from the type traits.
    ///
    /// F-TYPES-3 / #24: when the type carries a serialized COMPLETE
    /// `TypeObject` (`traits::type_object_len() > 0`, emitted by `idlc cpp`),
    /// the writer is created via `zerodds_writer_create_typed`, which registers
    /// the object and derives + installs the `TypeIdentifier` on the endpoint
    /// slot BEFORE it is published — no discovery window with a `None`
    /// identifier. Union topics or types whose TypeObject could not be lowered
    /// at codegen time fall back to the byte-oriented `create_kind`.
    TypedWriter(Runtime &rt, const std::string &topic_name, bool reliable = true) {
        const uint8_t *type_object = traits::type_object();
        uintptr_t type_object_len = traits::type_object_len();
        handle_ = (type_object && type_object_len > 0)
            ? zerodds_writer_create_typed(
                  rt.raw(), topic_name.c_str(), traits::type_name(),
                  reliable ? 1 : 0, traits::is_keyed() ? 1 : 0,
                  type_object, type_object_len)
            : zerodds_writer_create_kind(
                  rt.raw(), topic_name.c_str(), traits::type_name(),
                  reliable ? 1 : 0, traits::is_keyed() ? 1 : 0);
        if (!handle_) {
            throw StatusError(-1, "zerodds_writer_create_typed");
        }
        // The encapsulation identifier has to name the encoding of the body,
        // XTypes 1.3 §7.6.3.1.2 Table 59 / DDSI-RTPS 2.5 §10.5. Without this
        // call the writer slot keeps the C ABI's default, FINAL, and stamps
        // `PLAIN_CDR2_LE` (0x0007) on every sample — measured. An
        // `@appendable` body opens with a DHEADER and a `@mutable` one with an
        // EMHEADER per member, so under that identifier a reader that believes
        // the header reads the delimiter as the first member. Cyclone decodes
        // it anyway (it uses its own type, not the header); OpenDDS refuses the
        // sample in `to_encoding_i` — "expected APPENDABLE extensibility, but
        // got ..." — and drops it with nothing on the topic.
        int extensibility_rc =
            zerodds_writer_set_wire_extensibility(handle_, wire_extensibility());
        if (extensibility_rc != 0) {
            // Destroyed rather than kept: a writer that could not declare its
            // own framing publishes samples no strict peer accepts, and doing
            // that quietly is the failure this call exists to end.
            zerodds_writer_destroy(handle_);
            handle_ = nullptr;
            throw StatusError(extensibility_rc, "zerodds_writer_set_wire_extensibility");
        }
    }
    ~TypedWriter() {
        if (handle_) zerodds_writer_destroy(handle_);
    }
    ZERODDS_NONCOPYABLE(TypedWriter)
    TypedWriter(TypedWriter &&o) ZERODDS_NOEXCEPT : handle_(o.handle_) { o.handle_ = nullptr; }
    TypedWriter &operator=(TypedWriter &&o) ZERODDS_NOEXCEPT {
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

    /// Spec §2.2.2.4.2.10 `dispose`, keyed by `value`'s own `@key`
    /// members. Encodes `value` the same way `write()` does and hands
    /// the bytes to `zerodds_writer_dispose_sample`, which derives the
    /// instance's real KeyHash and `serializedKey` from them through this
    /// writer's registered `TypeObject` — the full form Eclipse Cyclone
    /// 11.0.1, Fast DDS 3.6.1 and OpenDDS 3.35 were all measured to act
    /// on (`Test_FinalInstanceState_1`, 4/4), unlike a bare `key_hash`
    /// marker (see `Writer::dispose(const uint8_t[16])`'s own doc
    /// comment). Throws `StatusError` (status `PreconditionNotMet`) if
    /// this writer fell back to `create_kind` (no registered
    /// `TypeObject`) or the type is unkeyed.
    void dispose(const T &value) {
        std::vector<uint8_t> bytes = traits::encode(value);
        int rc = zerodds_writer_dispose_sample(handle_, bytes.data(), bytes.size());
        if (rc != 0) throw StatusError(rc, "zerodds_writer_dispose_sample");
    }

    /// Spec §2.2.2.4.2.7 `unregister_instance`, keyed by `value` — see
    /// `dispose(const T &)`.
    void unregister_instance(const T &value) {
        std::vector<uint8_t> bytes = traits::encode(value);
        int rc = zerodds_writer_unregister_sample(handle_, bytes.data(), bytes.size());
        if (rc != 0) throw StatusError(rc, "zerodds_writer_unregister_sample");
    }

    /// Spec §2.2.3.21 with `autodispose=true`, keyed by `value` — see
    /// `dispose(const T &)`.
    void unregister_with_dispose(const T &value) {
        std::vector<uint8_t> bytes = traits::encode(value);
        int rc = zerodds_writer_unregister_with_dispose_sample(handle_, bytes.data(), bytes.size());
        if (rc != 0) throw StatusError(rc, "zerodds_writer_unregister_with_dispose_sample");
    }

private:
    /// The type's extensibility in the C ABI's own numbering:
    /// `0` FINAL, `1` APPENDABLE, `2` MUTABLE — the argument
    /// `zerodds_writer_set_wire_extensibility` takes.
    static int wire_extensibility() {
        typedef ::zerodds::psm::DataRepresentationKind Kind;
        switch (traits::extensibility()) {
        case Kind::APPENDABLE: return 1;
        case Kind::MUTABLE:    return 2;
        case Kind::FINAL:      break;
        }
        return 0;
    }

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
    typedef ::dds::topic::topic_type_support<T> traits;

public:
    /// F-TYPES-3 / #24: reader-side mirror of `TypedWriter` — a typed create
    /// (`zerodds_reader_create_typed`) when the type carries a serialized
    /// COMPLETE `TypeObject`, else the byte-oriented `create_kind` fallback.
    TypedReader(Runtime &rt, const std::string &topic_name, bool reliable = true) {
        const uint8_t *type_object = traits::type_object();
        uintptr_t type_object_len = traits::type_object_len();
        handle_ = (type_object && type_object_len > 0)
            ? zerodds_reader_create_typed(
                  rt.raw(), topic_name.c_str(), traits::type_name(),
                  reliable ? 1 : 0, traits::is_keyed() ? 1 : 0,
                  type_object, type_object_len)
            : zerodds_reader_create_kind(
                  rt.raw(), topic_name.c_str(), traits::type_name(),
                  reliable ? 1 : 0, traits::is_keyed() ? 1 : 0);
        if (!handle_) {
            throw StatusError(-1, "zerodds_reader_create_typed");
        }
    }
    ~TypedReader() {
        if (handle_) zerodds_reader_destroy(handle_);
    }
    ZERODDS_NONCOPYABLE(TypedReader)
    TypedReader(TypedReader &&o) ZERODDS_NOEXCEPT : handle_(o.handle_) { o.handle_ = nullptr; }
    TypedReader &operator=(TypedReader &&o) ZERODDS_NOEXCEPT {
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
        // `out_be` reports the wire byte order from the encapsulation header
        // (RTPS 2.5 §10.5) so a big-endian peer's sample decodes correctly.
        uint8_t be = 0;
        int rc = zerodds_reader_take_endian(handle_, &raw, &len, &repr, &be);
        if (rc != 0) throw StatusError(rc, "zerodds_reader_take_endian");
        if (!raw || len == 0) return false;
        auto version = repr == 1 ? ::zerodds::wire::Encoding::PlainCdr2
                                 : ::zerodds::wire::Encoding::PlainCdr1;
        try {
            out = traits::decode(raw, len, version, be != 0);
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
