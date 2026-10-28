/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * zerodds_xrce_client.h -- XRCE session client for the native C endpoint SDK
 * (ADR 0013), C89, no malloc.
 *
 * Two halves, kept apart on purpose (the same split `crates/xrce-client` and
 * `crates/xrce` keep):
 *
 *  1. The STATE MACHINE (`zdwc_client` + `zdwc_*` lifecycle calls below)
 *     mirrors `crates/xrce-client/src/lib.rs::XrceClient` field for field and
 *     operation for operation: `Disconnected -> Connecting -> Connected`,
 *     the same `next_request_id` counter (wraps 1..65535, never 0, via the
 *     same `+1, then max(1)` rule), the same four gated operations. Every
 *     call below cites the OMG DDS-XRCE 1.0 clause its Rust counterpart
 *     cites.
 *  2. The FRAMING (`zdwc_frame_*`) builds the actual octets -- the Rust
 *     state machine deliberately does not ("the caller delegates submessage
 *     encoding; we only track state"), but a port whose purpose is "a
 *     program in this language can talk to a ZeroDDS instance" has to
 *     produce real wire bytes somewhere, so this SDK provides that half
 *     too, in the same manual-byte-layout style `zdw_xrce_write_frame`
 *     (zerodds_endpoint.h) already uses -- not the generic `zdw_writer`,
 *     because these frames are fixed small structures, not generated
 *     codecs.
 *
 * `crates/xrce` treats the CREATE_CLIENT body's XCDR2 layout as
 * `crates/xrce/src/handshake.rs::ClientRepresentation` (Annex A,
 * `CLIENT_Representation`): xrce_cookie(4) + xrce_version(2) +
 * xrce_vendor_id(2) + client_key(4) + session_id(1) + a `0`
 * properties-absent octet + eProsima's own non-normative `requested_mtu`
 * extension (2, little endian) -- byte-oracled there against a real
 * eProsima Micro-XRCE-DDS Client 3.0.1 capture, not a hand-picked
 * placeholder. `zdwc_frame_create_client` below builds exactly that, with
 * the MessageHeader carrying the ClientKey (`session_id` is this client's
 * own choice, `<= 127` per `MessageHeader::with_client_key`).
 * Fixtures: `endpoints/testdata/xrce-client/ (*.bin)`.
 *
 * All multi-byte header/submessage fields are little-endian (spec
 * §8.3.2.3/§8.3.4); `ObjectId` is big-endian on the wire (spec Annex B
 * examples, `object_id.rs::to_bytes`). `request_id` (spec §7.7.8) is an
 * opaque `octet[2]` on the wire -- the wire itself has no byte order for
 * it, only crates/xrce-client's `u16` counter does, and `crates/xrce-client
 * ::request_id_octets` fixes this SDK's choice: big-endian (matching
 * ObjectId's own order and the spec's own Annex B examples, e.g.
 * "request_id = {0xAA, 0x01}").
 */

#ifndef ZERODDS_XRCE_CLIENT_H
#define ZERODDS_XRCE_CLIENT_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* --- lifecycle states (mirrors ClientState) --- */
#define ZDWC_DISCONNECTED 0
#define ZDWC_CONNECTING   1
#define ZDWC_CONNECTED    2

/* --- result codes (mirrors ClientError) --- */
#define ZDWC_OK               0
#define ZDWC_E_INVALID_STATE  1  /* operation in the wrong lifecycle state   */
#define ZDWC_E_DOES_NOT_FIT   2  /* zdwc_frame_* : the frame overran `cap`   */

/* ObjectKind codes actually exercised by this client (DDS-XRCE 1.0 Table 4,
 * `crates/xrce/src/object_kind.rs`). A full kind table belongs to a codegen
 * layer, not this hand-written session client. */
#define ZDWC_OBJK_PARTICIPANT 0x01
#define ZDWC_OBJK_TOPIC       0x02
#define ZDWC_OBJK_PUBLISHER   0x03
#define ZDWC_OBJK_SUBSCRIBER  0x04
#define ZDWC_OBJK_DATAWRITER  0x05
#define ZDWC_OBJK_DATAREADER  0x06

/* Packs a 12-bit raw_id + 4-bit ObjectKind into the 16-bit wire ObjectId
 * (`object_id.rs::ObjectId::new`). raw_id above 0xFFF is truncated by the
 * caller's responsibility -- this header has no error channel for it,
 * matching the fixed-width, no-malloc style of the rest of this SDK. */
#define ZDWC_OBJECT_ID(raw_id, kind) \
    ((unsigned int)(((raw_id) << 4) | ((kind) & 0x0Fu)))

/* --- state machine (mirrors crates/xrce-client::XrceClient) --- */

typedef struct zdwc_client {
    unsigned char client_key[4];
    int state;                    /* ZDWC_DISCONNECTED/CONNECTING/CONNECTED */
    unsigned char session_id;     /* proposed at connect(), spec-adopted
                                    * verbatim by the agent (§7.8.2.1: "If
                                    * the Agent creates a ProxyClient object
                                    * it shall initialize its state to have
                                    * the specified session_id") -- valid
                                    * once mark_connected() has been called */
    unsigned int next_request_id; /* 1..65535, wraps skipping 0             */
    int representation_timestamps; /* ZDWC_REPRESENTATION_TIMESTAMPS_*      */
    int unannotated_types;         /* ZDWC_UNANNOTATED_*                    */
} zdwc_client;

/* Constructor. §7.8.2 CREATE_CLIENT (via connect() below); this call only
 * initialises local state -- Disconnected, next_request_id = 1. */
void zdwc_client_init(zdwc_client *c, const unsigned char client_key[4]);

/* Setting "representation timestamps" (DDS-XRCE 1.0 7.8.2.1): the prose
 * lists `Time_t client_timestamp` in CLIENT_Representation after
 * xrce_vendor_id; the IDL of Annex A and the bytes of Annex B.1 do not have it.
 * ZDWC_REPRESENTATION_TIMESTAMPS_ABSENT (0, default) writes the Annex A form
 * (16 octets), ZDWC_REPRESENTATION_TIMESTAMPS_PRESENT (1) the prose form.
 * The agent has to be set the same way (`representation_timestamps:` in its
 * configuration).
 *
 * Time_t carries no @extensibility, so it is APPENDABLE (DDS-XTypes 1.3
 * 7.3.1.2.1.8): DHEADER (u32 = 8) + seconds (i32) + nanoseconds (u32), 12
 * octets, 28 octets for the whole CLIENT_Representation. With the setting
 * "unannotated types" ZDWC_UNANNOTATED_FINAL (1) there is no DHEADER (8 octets,
 * 24 octets in all); ZDWC_UNANNOTATED_APPENDABLE (0) is the default. The
 * values equal ZDW_UNANNOTATED_* of zerodds_endpoint.h. */
#define ZDWC_REPRESENTATION_TIMESTAMPS_ABSENT  0
#define ZDWC_REPRESENTATION_TIMESTAMPS_PRESENT 1
#define ZDWC_UNANNOTATED_APPENDABLE            0
#define ZDWC_UNANNOTATED_FINAL                 1

/* Sets the two settings of this client; used by
 * zdwc_client_frame_create_client. Defaults after zdwc_client_init: ABSENT,
 * APPENDABLE. */
void zdwc_set_representation_timestamps(zdwc_client *c, int representation_timestamps,
                                        int unannotated_types);

/* Current lifecycle state. */
int zdwc_state(const zdwc_client *c);

/* Initiates the handshake: proposes `requested_session_id` (1..127, spec
 * §8.3.2.1 -- a ClientKey-carrying session) and moves Disconnected ->
 * Connecting. §8.4.5 (Handshake). Returns ZDWC_E_INVALID_STATE if already
 * connected/connecting. This call does not touch the wire -- see
 * zdwc_frame_create_client for the CREATE_CLIENT octets. */
int zdwc_connect(zdwc_client *c, unsigned char requested_session_id);

/* Marks the handshake complete -- the caller has received STATUS_AGENT
 * (recognise it by submessage_id == 4, spec §8.3.5.5; this client does not
 * decode AGENT_Representation, matching crates/xrce-client's own scope).
 * §8.4.5. Returns ZDWC_E_INVALID_STATE unless Connecting. */
int zdwc_mark_connected(zdwc_client *c);

/* Submits a CREATE operation: gates on Connected and returns the next
 * request_id. §7.8.3 CREATE. Returns ZDWC_E_INVALID_STATE otherwise;
 * `*out_request_id` is unset in that case. */
int zdwc_create_object(zdwc_client *c, unsigned int *out_request_id);

/* Submits a DELETE operation. §7.8.3 DELETE. */
int zdwc_delete_object(zdwc_client *c, unsigned int *out_request_id);

/* Submits a WRITE_DATA operation. §7.8.4 WRITE_DATA. */
int zdwc_request_write(zdwc_client *c, unsigned int *out_request_id);

/* Submits a READ_DATA operation (pull-based). §7.8.5 READ_DATA. */
int zdwc_request_read(zdwc_client *c, unsigned int *out_request_id);

/* Closes the session -- back to Disconnected. §8.4.5. Local state only,
 * exactly like crates/xrce-client::disconnect: no DELETE_CLIENT is sent. */
void zdwc_disconnect(zdwc_client *c);

/* eProsima's own hard datagram ceiling (crates/xrce/src/handshake.rs's
 * module doc has the byte-oracle measurement backing it), used as this
 * SDK's own CREATE_CLIENT requested_mtu. */
#define ZDWC_DEFAULT_MTU 508u

/* --- framing (the octets a real transport sends) --- */

/* CREATE_CLIENT (id=0x00): MessageHeader with session_id =
 * SESSION_ID_NONE_WITH_CLIENT_KEY (0x00) plus the ClientKey when the
 * requested `session_id` is 0..127, or SESSION_ID_NONE_WITHOUT_CLIENT_KEY
 * (0x80) without ClientKey when it is 128..255 (DDS-XRCE 1.0 §8.3.2.1; the
 * requested `session_id` itself is in the CLIENT_Representation payload;
 * stream_id=0x00 STREAMID_NONE, sequence_nr=0) + one CREATE_CLIENT
 * submessage (flags=0x01, E-flag only -- CreateClientPayload::
 * into_submessage hardcodes this, no reuse/replace bits) whose body is a
 * real CLIENT_Representation (see the header doc comment) -- byte-identical
 * to `endpoints/testdata/xrce-client/xrce_client_create_client.bin`.
 * Returns the frame length, or 0 if it does not fit `cap`. */
size_t zdwc_frame_create_client(unsigned char session_id,
                                const unsigned char client_key[4],
                                unsigned char *out, size_t cap);

/* As zdwc_frame_create_client, with the settings and the Time_t as explicit
 * parameters: `representation_timestamps` (ZDWC_REPRESENTATION_TIMESTAMPS_*),
 * `unannotated_types` (ZDWC_UNANNOTATED_*, only read when the timestamp is
 * present), `seconds` / `nanoseconds` (little endian on the wire, only read
 * when the timestamp is present). ABSENT gives the same 28 octets as
 * zdwc_frame_create_client. PRESENT gives 40 octets (APPENDABLE) or 36 octets
 * (FINAL); the submessage length is 28 or 24. */
size_t zdwc_frame_create_client_with(unsigned char session_id,
                                     const unsigned char client_key[4],
                                     int representation_timestamps,
                                     int unannotated_types,
                                     long seconds, unsigned long nanoseconds,
                                     unsigned char *out, size_t cap);

/* CREATE_CLIENT of this client: zdwc_frame_create_client_with with the
 * settings of `c` (zdwc_set_representation_timestamps) and, when the
 * timestamp is present, the wall clock (time(), nanoseconds 0) at the time of
 * the call. */
size_t zdwc_client_frame_create_client(const zdwc_client *c, unsigned char session_id,
                                       unsigned char *out, size_t cap);

/* CREATE(REPRESENTATION_BY_REFERENCE) on an open session: MessageHeader
 * WITH the ClientKey (session_id <= 127, §8.3.2.1) + one CREATE submessage
 * (§7.7.8 BaseObjectRequest + the real, two-level §7.7.3 ObjectVariant:
 * the outer `object_kind` byte, then the inner REPRESENTATION_BY_REFERENCE
 * payload, then that kind's own trailing field -- `parent_object_id`
 * (when `has_parent` is nonzero) for a
 * Topic/Publisher/Subscriber/DataWriter/DataReader, absent for every
 * other kind -- byte-oracled in `crates/xrce/src/object_repr.rs` against
 * real eProsima Micro-XRCE-DDS Client 3.0.1 CREATE traffic (Topic's own
 * participant_id: commit 31c62df2a63). `reference` is a
 * NUL-terminated object_reference string (spec §7.7.3.1.1). `request_id`
 * is encoded big-endian (2 bytes), matching ObjectId's own wire byte order
 * and the spec's own Annex B examples. Returns the frame length, or 0 if
 * it does not fit. */
size_t zdwc_frame_create(unsigned char session_id,
                         const unsigned char client_key[4],
                         unsigned int seq, unsigned int request_id,
                         unsigned int object_id, unsigned int object_kind,
                         const char *reference,
                         int has_parent, unsigned int parent_object_id,
                         int reuse, int replace,
                         unsigned char *out, size_t cap);

/* DELETE on an open session: MessageHeader WITH the ClientKey + one DELETE
 * submessage (§7.7.8 BaseObjectRequest only, `submessages/delete.rs`).
 * Returns the frame length, or 0 if it does not fit. */
size_t zdwc_frame_delete(unsigned char session_id,
                         const unsigned char client_key[4],
                         unsigned int seq, unsigned int request_id,
                         unsigned int object_id,
                         unsigned char *out, size_t cap);

/* WRITE_DATA (FORMAT_DATA, §8.3.5.8): MessageHeader WITH the ClientKey +
 * one WRITE_DATA submessage (§7.7.8 BaseObjectRequest + the raw sample
 * body, verbatim, no length of its own -- `submessages/write_data.rs`).
 * Distinct from `zdw_xrce_write_frame` (zerodds_endpoint.h), which frames
 * the non-conformant "ZeroDDS Endpoint Profile" no-key WRITE_DATA the
 * direct-publish ports still speak. Returns the frame length, or 0 if it
 * does not fit or `sample_len > 0xFFFF`. */
size_t zdwc_frame_write_data(unsigned char session_id,
                             const unsigned char client_key[4],
                             unsigned int seq, unsigned int request_id,
                             unsigned int writer_object_id,
                             const unsigned char *sample, size_t sample_len,
                             unsigned char *out, size_t cap);

/* READ_DATA (§8.3.5.9): MessageHeader WITH the ClientKey + one READ_DATA
 * submessage (§7.7.8 BaseObjectRequest + a minimal ReadSpecification: no
 * content filter, no delivery control -- both @optional members absent,
 * `submessages/read_data.rs`). `preferred_stream_id` selects the StreamId
 * the agent should use for the DATA reply (0 = STREAMID_NONE, "leave the
 * decision to the Agent", spec Annex B.4.1). Returns the frame length, or 0
 * if it does not fit. */
size_t zdwc_frame_read_data(unsigned char session_id,
                            const unsigned char client_key[4],
                            unsigned int seq, unsigned int request_id,
                            unsigned int reader_object_id,
                            unsigned char preferred_stream_id,
                            unsigned char *out, size_t cap);

#ifdef __cplusplus
}
#endif

#endif /* ZERODDS_XRCE_CLIENT_H */
