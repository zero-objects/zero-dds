// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds_xrce_client.hpp -- C++98 facade over the XRCE session client
// (endpoints/c/include/zerodds_xrce_client.h).
//
// Same relationship as zerodds_wire.hpp to zerodds_wire.h: this header does
// NOT re-implement the state machine or the framing -- it wraps the proven
// C89 `zdwc_*` functions, so it cannot drift from them or from the fixtures
// they already reproduce byte-identically
// (endpoints/testdata/xrce-client/*.bin). No C++11+, no exceptions thrown
// by this header itself -- errors are the same ZDWC_* codes the C API
// returns, checked the same way zerodds_wire.hpp's Writer/Reader report
// theirs (`error()`), consistent with this SDK's no-malloc, no-throw
// style.
//
// Spec mapping (mirrors crates/xrce-client/src/lib.rs's module doc and
// zerodds_xrce_client.h's own):
//   XrceClient ctor -> §7.8.2 (CREATE_CLIENT)
//   connect          -> §8.4.5 (Handshake)
//   create_object    -> §7.8.3 (CREATE)
//   delete_object    -> §7.8.3 (DELETE)
//   request_write    -> §7.8.4 (WRITE_DATA)
//   request_read     -> §7.8.5 (READ_DATA)
//   disconnect       -> §8.4.5

#ifndef ZERODDS_XRCE_CLIENT_HPP
#define ZERODDS_XRCE_CLIENT_HPP

extern "C" {
#include "zerodds_xrce_client.h"
}

#include <string>
#include <vector>

namespace zerodds {

// A framed message never exceeds this in the fixtures or in any operation
// this SDK exposes (the largest, CREATE with a long object_reference, is
// bounded by the caller's own string). 512 covers every realistic
// reference/sample this thin facade is meant for; callers needing more
// should call the C `zdwc_frame_*` functions directly with a larger buffer.
enum { XRCE_FRAME_CAP = 512 };

// Result codes: mirrors ZDWC_OK / ZDWC_E_INVALID_STATE / ZDWC_E_DOES_NOT_FIT
// (zerodds_xrce_client.h). Kept as plain ints, not an enum class (C++98).

namespace xrce {

// Setting "representation timestamps" (DDS-XRCE 1.0 7.8.2.1): the prose lists
// `Time_t client_timestamp` in CLIENT_Representation after xrce_vendor_id; the
// IDL of Annex A and the bytes of Annex B.1 do not. Absent is the default
// (Annex A form, 16 octets). The agent has to be set the same way
// (`representation_timestamps:` in its configuration).
namespace RepresentationTimestamps {
enum Value { Absent = ZDWC_REPRESENTATION_TIMESTAMPS_ABSENT,
             Present = ZDWC_REPRESENTATION_TIMESTAMPS_PRESENT };
}

// Setting "unannotated types" for Time_t (no @extensibility, APPENDABLE by
// DDS-XTypes 1.3 7.3.1.2.1.8): Appendable (default) writes a DHEADER = 8 in
// front of the Time_t, Final does not. Only read when the timestamp is present.
namespace UnannotatedTypes {
enum Value { Appendable = ZDWC_UNANNOTATED_APPENDABLE,
             Final = ZDWC_UNANNOTATED_FINAL };
}

}  // namespace xrce

// Synchronous XRCE session client -- a thin wrapper around `zdwc_client`.
// Two halves, kept apart on purpose (the same split crates/xrce-client and
// crates/xrce keep, and the C API keeps too): the STATE MACHINE (this
// class) and the FRAMING (the free frame_* functions below).
class XrceClient {
public:
    // `representation_timestamps` (xrce::RepresentationTimestamps::Absent or
    // Present) and `unannotated_types` (xrce::UnannotatedTypes::Appendable or
    // Final) are used by create_client_frame(). Defaults: Absent, Appendable.
    explicit XrceClient(const unsigned char client_key[4],
                        int representation_timestamps = ZDWC_REPRESENTATION_TIMESTAMPS_ABSENT,
                        int unannotated_types = ZDWC_UNANNOTATED_APPENDABLE) {
        zdwc_client_init(&c_, client_key);
        zdwc_set_representation_timestamps(&c_, representation_timestamps, unannotated_types);
    }

    // CREATE_CLIENT frame with the settings of this client; with timestamps
    // present, the wall clock at the time of the call. Empty if it did not fit.
    std::vector<unsigned char> create_client_frame(unsigned char session_id) const {
        unsigned char buf[XRCE_FRAME_CAP];
        size_t n = zdwc_client_frame_create_client(&c_, session_id, buf, sizeof(buf));
        return std::vector<unsigned char>(buf, buf + n);
    }

    // Current lifecycle state: ZDWC_DISCONNECTED/CONNECTING/CONNECTED.
    int state() const { return zdwc_state(&c_); }

    const unsigned char* client_key() const { return c_.client_key; }
    unsigned char session_id() const { return c_.session_id; }

    // §8.4.5 Handshake: proposes `requested_session_id` and moves
    // Disconnected -> Connecting. Returns ZDWC_E_INVALID_STATE if already
    // connected/connecting.
    int connect(unsigned char requested_session_id) {
        return zdwc_connect(&c_, requested_session_id);
    }

    // Marks the handshake complete once the caller has received
    // STATUS_AGENT. §8.4.5.
    int mark_connected() { return zdwc_mark_connected(&c_); }

    // §7.8.3 CREATE. Returns ZDWC_E_INVALID_STATE if not connected;
    // `*out_request_id` is unset in that case.
    int create_object(unsigned int* out_request_id) {
        return zdwc_create_object(&c_, out_request_id);
    }

    // §7.8.3 DELETE.
    int delete_object(unsigned int* out_request_id) {
        return zdwc_delete_object(&c_, out_request_id);
    }

    // §7.8.4 WRITE_DATA.
    int request_write(unsigned int* out_request_id) {
        return zdwc_request_write(&c_, out_request_id);
    }

    // §7.8.5 READ_DATA (pull-based).
    int request_read(unsigned int* out_request_id) {
        return zdwc_request_read(&c_, out_request_id);
    }

    // §8.4.5: closes the session -- back to Disconnected. Local state only,
    // like crates/xrce-client::disconnect -- no DELETE_CLIENT is sent.
    void disconnect() { zdwc_disconnect(&c_); }

private:
    zdwc_client c_;
};

namespace xrce {

// Framing: thin wrappers over `zdwc_frame_*` that own their own buffer and
// return the produced bytes, or an empty vector if the frame did not fit
// XRCE_FRAME_CAP (mirrors the C API's `0` return).

inline std::vector<unsigned char> frame_create_client(unsigned char session_id,
                                                       const unsigned char client_key[4]) {
    unsigned char buf[XRCE_FRAME_CAP];
    size_t n = zdwc_frame_create_client(session_id, client_key, buf, sizeof(buf));
    return std::vector<unsigned char>(buf, buf + n);
}

// As frame_create_client, with the settings and the Time_t as explicit values.
// seconds / nanoseconds are only read with RepresentationTimestamps::Present.
inline std::vector<unsigned char> frame_create_client(unsigned char session_id,
                                                       const unsigned char client_key[4],
                                                       int representation_timestamps,
                                                       int unannotated_types,
                                                       long seconds, unsigned long nanoseconds) {
    unsigned char buf[XRCE_FRAME_CAP];
    size_t n = zdwc_frame_create_client_with(session_id, client_key, representation_timestamps,
                                             unannotated_types, seconds, nanoseconds,
                                             buf, sizeof(buf));
    return std::vector<unsigned char>(buf, buf + n);
}

// `has_parent`/`parent_object_id` are this kind's own trailing field in
// the real, two-level ObjectVariant (spec §7.7.3) -- a parent id for a
// Topic/Publisher/Subscriber/DataWriter/DataReader, absent for every
// other kind (Topic's own participant_id: commit 31c62df2a63). See
// zerodds_xrce_client.h's zdwc_frame_create doc.
inline std::vector<unsigned char> frame_create(unsigned char session_id,
                                                const unsigned char client_key[4],
                                                unsigned int seq, unsigned int request_id,
                                                unsigned int object_id, unsigned int object_kind,
                                                const std::string& reference,
                                                bool has_parent, unsigned int parent_object_id,
                                                bool reuse, bool replace) {
    unsigned char buf[XRCE_FRAME_CAP];
    size_t n = zdwc_frame_create(session_id, client_key, seq, request_id, object_id,
                                 object_kind, reference.c_str(),
                                 has_parent ? 1 : 0, parent_object_id,
                                 reuse ? 1 : 0, replace ? 1 : 0,
                                 buf, sizeof(buf));
    return std::vector<unsigned char>(buf, buf + n);
}

inline std::vector<unsigned char> frame_delete(unsigned char session_id,
                                                const unsigned char client_key[4],
                                                unsigned int seq, unsigned int request_id,
                                                unsigned int object_id) {
    unsigned char buf[XRCE_FRAME_CAP];
    size_t n = zdwc_frame_delete(session_id, client_key, seq, request_id, object_id,
                                 buf, sizeof(buf));
    return std::vector<unsigned char>(buf, buf + n);
}

inline std::vector<unsigned char> frame_write_data(unsigned char session_id,
                                                    const unsigned char client_key[4],
                                                    unsigned int seq, unsigned int request_id,
                                                    unsigned int writer_object_id,
                                                    const std::vector<unsigned char>& sample) {
    unsigned char buf[XRCE_FRAME_CAP];
    const unsigned char* sample_ptr = sample.empty() ? 0 : &sample[0];
    size_t n = zdwc_frame_write_data(session_id, client_key, seq, request_id,
                                     writer_object_id, sample_ptr, sample.size(),
                                     buf, sizeof(buf));
    return std::vector<unsigned char>(buf, buf + n);
}

inline std::vector<unsigned char> frame_read_data(unsigned char session_id,
                                                   const unsigned char client_key[4],
                                                   unsigned int seq, unsigned int request_id,
                                                   unsigned int reader_object_id,
                                                   unsigned char preferred_stream_id) {
    unsigned char buf[XRCE_FRAME_CAP];
    size_t n = zdwc_frame_read_data(session_id, client_key, seq, request_id,
                                    reader_object_id, preferred_stream_id,
                                    buf, sizeof(buf));
    return std::vector<unsigned char>(buf, buf + n);
}

} // namespace xrce
} // namespace zerodds

#endif // ZERODDS_XRCE_CLIENT_HPP
