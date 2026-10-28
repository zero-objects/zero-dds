/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * zerodds_endpoint.h -- the Frame-Hook contract (ADR 0013, invariant 5).
 *
 * The endpoint SDK is transport-opaque: it hands a fully-framed, encoded
 * message to the integrator's transport and receives complete frames back. The
 * integrator fills `zdw_transport` for their environment -- a UDP or serial
 * socket, a bus mailbox, whatever the platform has. Nothing about the OS or the
 * wire below the frame is baked in; that is the single mandatory integration
 * point.
 *
 * The endpoint speaks to a Rust hub (the XRCE agent); the frame payload is an
 * XCDR-encoded sample produced by the generated wire-fixed codec. This header
 * is C89, no malloc, no external libs -- like the wire-core.
 */

#ifndef ZERODDS_ENDPOINT_H
#define ZERODDS_ENDPOINT_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Transport result codes. */
#define ZDW_T_OK        0
#define ZDW_T_AGAIN     1  /* receive: no frame currently available */
#define ZDW_T_ERROR     2  /* transport failure                     */

/* A transport the endpoint delivers framed messages to and receives them from.
 * The integrator provides `ctx` + the two function pointers; the endpoint never
 * assumes anything about how a frame reaches the peer. */
typedef struct zdw_transport {
    void *ctx;
    /* Deliver one complete frame (`len` bytes) to the peer. Returns ZDW_T_OK
     * on success, ZDW_T_ERROR on failure. */
    int (*deliver)(void *ctx, const unsigned char *frame, size_t len);
    /* Receive one complete frame into `buf` (capacity `cap`); on ZDW_T_OK
     * `*len` is the frame length. ZDW_T_AGAIN when none is available. */
    int (*receive)(void *ctx, unsigned char *buf, size_t cap, size_t *len);
} zdw_transport;

/* Convenience: deliver a frame through the transport. */
int zdw_endpoint_send(const zdw_transport *t, const unsigned char *frame, size_t len);
/* Convenience: receive a frame from the transport. */
int zdw_endpoint_recv(const zdw_transport *t, unsigned char *buf, size_t cap, size_t *len);

/* --- DDS-XRCE framing (OMG DDS-XRCE 1.0) ---
 *
 * The endpoint↔hub protocol: an XCDR-encoded sample body is wrapped in an XRCE
 * WRITE_DATA message that a `crates/xrce` agent accepts. Message header
 * (session_id, stream_id, sequence_nr LE) + WRITE_DATA submessage header
 * (id=7, flags, length LE) + the sample body. Best-effort, no ClientKey
 * (session_id >= 128). Header + submessage-header are always little-endian
 * (spec §8.3.2.3/§8.3.4); the sample body's byte order is carried by the E flag. */

/* Reserved best-effort session without a ClientKey (session_id >= 128). */
#define ZDW_XRCE_SESSION_NOKEY  0x80
/* Built-in best-effort stream. */
#define ZDW_XRCE_STREAM_BEST_EFFORT 0x01

/* Submessage flags of a data submessage: E flag (little-endian body, bit 0) plus
 * DataFormat FORMAT_DATA (0b000) in bits 1..3 -- DDS-XRCE 1.0 Sec 8.3.5.8. */
#define ZDW_XRCE_WRITE_FLAGS 0x01
/* Submessage flags of a control submessage (ACKNACK/HEARTBEAT): the E flag
 * only -- they define no format bits. */
#define ZDW_XRCE_FLAG_E_LE 0x01
/* Wire size of the BaseObjectRequest that precedes the sample (Sec 7.7.8). */
#define ZDW_XRCE_BASE_SIZE 4u
/* ObjectKind of a DataWriter (Sec 7.2.2, OBJK_DATAWRITER). */
#define ZDW_OBJK_DATAWRITER 0x05u
/* Builds an ObjectId (Sec 7.2.1) from a 12-bit raw id and a 4-bit ObjectKind. */
#define ZDW_XRCE_OBJECT_ID(raw, kind) \
    ((unsigned int)((((unsigned int)(raw) & 0x0FFFu) << 4) | ((unsigned int)(kind) & 0x0Fu)))

/* Wraps `sample` (an XCDR body) into an XRCE WRITE_DATA frame in `out`.
 * The submessage body starts with the BaseObjectRequest (Sec 7.7.8):
 * `request_id` followed by `object_id`, each two octets, most significant
 * first. `object_id` identifies the target DataWriter -- without it no XRCE
 * agent can route the sample. `request_id` is the message's own `seq`, which
 * Sec 7.7.8 only requires to be unique per stream. Returns the frame length,
 * or 0 if it does not fit or `object_id` exceeds 16 bits. */
size_t zdw_xrce_write_frame(unsigned char *out, size_t cap,
                            unsigned char session, unsigned char stream,
                            unsigned int seq, unsigned int object_id,
                            const unsigned char *sample,
                            size_t sample_len);

/* Locates the sample body inside a received WRITE_DATA/DATA frame -- i.e. the
 * bytes AFTER the 4-octet BaseObjectRequest. On success sets *body / *body_len
 * and returns ZDW_T_OK; ZDW_T_ERROR if the frame is not a best-effort no-key
 * WRITE_DATA/DATA or its body is too short to hold the BaseObjectRequest. */
int zdw_xrce_read_frame(const unsigned char *frame, size_t len,
                        const unsigned char **body, size_t *body_len);

/* Reads the BaseObjectRequest of a received WRITE_DATA/DATA frame into
 * *request_id / *object_id (either may be NULL). Same acceptance rules as
 * `zdw_xrce_read_frame`. */
int zdw_xrce_frame_base(const unsigned char *frame, size_t len,
                        unsigned int *request_id, unsigned int *object_id);

/* --- XRCE serial framing (DDS-XRCE 1.0 Annex C.1) ---
 *
 * Serial links have no frame boundaries, so an XRCE message is wrapped
 * (Annex C.1.1):
 *   7E | sourceAddress | remoteAddress | payloadLength (2, LE) | payload |
 *   frameCheck (2, LE)
 * Every byte after the flag that is 0x7E or 0x7D becomes 0x7D followed by
 * (byte XOR 0x20) (C.1.2). frameCheck is the FCS-16 of RFC 1662 (polynomial
 * x^16 + x^12 + x^5 + 1, reflected, init 0xFFFF, complemented) over the
 * addresses, payloadLength and payload (C.1.1.6). This is the framing a
 * `crates/xrce` serial transport exchanges -- the fill for the frame-hook on
 * a serial line. */

#define ZDW_SERIAL_HEADER_LEN 4u

/* FCS-16 of RFC 1662 over `data`; 0x906E for "123456789". */
unsigned int zdw_fcs16(const unsigned char *data, size_t n);
/* Frames `payload` from `source` to `remote` into `out`. Returns the frame
 * length, 0 if it does not fit (worst case 1 + 2*(n+6) bytes) or n > 65535. */
size_t zdw_serial_frame(unsigned char *out, size_t cap,
                        unsigned char source, unsigned char remote,
                        const unsigned char *payload, size_t n);
/* De-frames the frame at the start of `in` (beginning with its 0x7E): destuffs,
 * checks frameCheck, writes the payload to `out`. ZDW_T_ERROR on a check
 * mismatch, a bad escape, a 0x7E before the frame is complete (C.1.3), or a
 * frame that ends with `in`. On ZDW_T_OK *out_len, *source and *remote are set
 * (either pointer may be null), and *consumed (if not null) is the number of
 * bytes of `in` the frame took. */
int zdw_serial_deframe(const unsigned char *in, size_t len,
                       unsigned char *source, unsigned char *remote,
                       unsigned char *out, size_t cap, size_t *out_len,
                       size_t *consumed);

/* --- reliable stream (DDS-XRCE HEARTBEAT / ACKNACK) ---
 *
 * On a reliable stream (stream_id >= 128) the agent periodically sends a
 * HEARTBEAT (first/last unacked sequence) and the endpoint replies with an
 * ACKNACK (first-unacked + a 16-bit NACK bitmap of missing samples). The
 * HEARTBEAT body is 5 octets LE (HEARTBEAT_Payload is FINAL in Annex A).
 * ACKNACK_Payload is APPENDABLE (no @extensibility in DDS-XRCE 1.0 Annex A;
 * DDS-XTypes 1.3 7.3.1.2.1.8), so by default its body is 9 octets: a 4-octet
 * DHEADER (= 5) followed by the 5 octets.
 *
 * Setting "unannotated types": ZDW_UNANNOTATED_APPENDABLE (0, default) or
 * ZDW_UNANNOTATED_FINAL (1; no DHEADER, body 5 octets, 13-octet frame, the form
 * of the reference implementation). It is chosen per reliable stream
 * (`zdw_reliable_init_with`) or per call (`zdw_xrce_acknack_frame_with`).
 * ACKNACK_Payload is the only type without @extensibility this SDK writes
 * (the CREATE_CLIENT builder writes no Property). The readers accept both
 * forms in either setting. */
#define ZDW_UNANNOTATED_APPENDABLE 0
#define ZDW_UNANNOTATED_FINAL      1

/* Builds an ACKNACK message. `nack` is the little-endian 16-bit bitmap (0 = all
 * received). Writes the 17-octet frame (9-octet body with DHEADER) and returns
 * its length, or 0 if it does not fit. Same as `zdw_xrce_acknack_frame_with`
 * with ZDW_UNANNOTATED_APPENDABLE. */
size_t zdw_xrce_acknack_frame(unsigned char *out, size_t cap,
                              unsigned char session, unsigned char stream,
                              unsigned int seq, int first_unacked,
                              unsigned char nack_lo, unsigned char nack_hi,
                              unsigned char payload_stream);
/* As zdw_xrce_acknack_frame, with the unannotated-types setting as an explicit
 * parameter: ZDW_UNANNOTATED_APPENDABLE writes the 17-octet frame,
 * ZDW_UNANNOTATED_FINAL the 13-octet frame (body length 5, no DHEADER). */
size_t zdw_xrce_acknack_frame_with(unsigned char *out, size_t cap,
                                   unsigned char session, unsigned char stream,
                                   unsigned int seq, int first_unacked,
                                   unsigned char nack_lo, unsigned char nack_hi,
                                   unsigned char payload_stream,
                                   int unannotated_types);
/* Parses a HEARTBEAT frame: *first / *last unacked sequence, *stream. */
int zdw_xrce_heartbeat_read(const unsigned char *frame, size_t len,
                            int *first, int *last, unsigned char *stream);

/* Builds a HEARTBEAT message (the reliable sender announces its unacked window
 * to the peer). Same header layout as `zdw_xrce_acknack_frame` with id
 * 0x0B and a 5-byte body (no DHEADER) (first i16 LE, last i16 LE, stream). Returns the frame
 * length, or 0 if it does not fit. */
size_t zdw_xrce_heartbeat_frame(unsigned char *out, size_t cap,
                                unsigned char session, unsigned char stream,
                                unsigned int seq, int first, int last,
                                unsigned char payload_stream);
/* Parses an ACKNACK frame: *first_unacked, *nack (16-bit LE bitmap), *stream.
 * Accepts body length 9 (DHEADER + 5) and body length 5 (FINAL form), in either
 * unannotated-types setting. */
int zdw_xrce_acknack_read(const unsigned char *frame, size_t len,
                          int *first_unacked, unsigned int *nack,
                          unsigned char *stream);

/* --- reliable stream state machine (DDS-XRCE 1.0 §8.4.10/§8.4.11) ---
 *
 * A stateful reliable sender + receiver mirroring `crates/xrce`
 * (`ReliableStreamState`), on the byte-identical HEARTBEAT/ACKNACK wire above.
 * Fixed storage, no malloc: sender window ZDW_REL_WINDOW, receiver reorder
 * buffer ZDW_REL_RECV_BUF, per-sample payload bounded to ZDW_REL_SLOT_CAP (the
 * embedded bound; the u16 wire limit is 65535). RFC-1982 16-bit sequence
 * numbers. A `zdw_reliable` is ~40 KiB -- allocate it static or on the heap,
 * not on a small stack. */

#define ZDW_REL_WINDOW       16
#define ZDW_REL_RECV_BUF     64
#define ZDW_REL_SLOT_CAP     512
#define ZDW_REL_HEARTBEAT_MS 500u

/* zdw_reliable_* result codes (distinct from ZDW_T_*). */
#define ZDW_REL_OK          0
#define ZDW_REL_TOO_LARGE   1  /* payload > ZDW_REL_SLOT_CAP */
#define ZDW_REL_WINDOW_FULL 2  /* sender window full         */
#define ZDW_REL_BUF_FULL    3  /* receiver reorder buffer full */

/* One buffered sample (sender in-flight or receiver out-of-order). */
typedef struct zdw_rel_slot {
    unsigned char used;
    unsigned short seq;
    unsigned short len;
    unsigned char buf[ZDW_REL_SLOT_CAP];
} zdw_rel_slot;

/* Reliable stream state (one per reliable stream). */
typedef struct zdw_reliable {
    unsigned char stream_id;
    /* sender */
    unsigned short next_seq;
    unsigned long last_hb_ms;
    int hb_started;
    zdw_rel_slot in_flight[ZDW_REL_WINDOW];
    /* receiver */
    unsigned short expected_seq;
    zdw_rel_slot recv_buf[ZDW_REL_RECV_BUF];
    /* ZDW_UNANNOTATED_APPENDABLE (default) / ZDW_UNANNOTATED_FINAL: the form of
     * the ACKNACK frames built by zdw_reliable_acknack_frame. */
    int unannotated_types;
} zdw_reliable;

/* Initialise / reset the whole state. */
void zdw_reliable_init(zdw_reliable *r, unsigned char stream_id);
/* As zdw_reliable_init, with the unannotated-types setting
 * (ZDW_UNANNOTATED_APPENDABLE or ZDW_UNANNOTATED_FINAL). zdw_reliable_init means
 * ZDW_UNANNOTATED_APPENDABLE. zdw_reliable_reset keeps the setting. */
void zdw_reliable_init_with(zdw_reliable *r, unsigned char stream_id,
                            int unannotated_types);
void zdw_reliable_reset(zdw_reliable *r);
int zdw_reliable_unannotated_types(const zdw_reliable *r);

/* -- sender side -- */
/* Buffers a new outgoing sample and assigns its RFC-1982 sequence number
 * (*out_seq). ZDW_REL_TOO_LARGE if len > ZDW_REL_SLOT_CAP, ZDW_REL_WINDOW_FULL
 * if the window already holds ZDW_REL_WINDOW in-flight samples. */
int zdw_reliable_submit(zdw_reliable *r, const unsigned char *payload,
                        size_t len, unsigned short *out_seq);
size_t zdw_reliable_in_flight_count(const zdw_reliable *r);
/* In-flight payload for `seq` (for retransmit), or 0 if not held. */
const unsigned char *zdw_reliable_get_in_flight(const zdw_reliable *r,
                                                unsigned short seq, size_t *len);
/* Returns 1 and sets first/last if a HEARTBEAT is due (in-flight non-empty and
 * ZDW_REL_HEARTBEAT_MS elapsed since the last, or the first ever), else 0.
 * `now_ms` is a monotonic millisecond clock. */
int zdw_reliable_pending_heartbeat(zdw_reliable *r, unsigned long now_ms,
                                   int *first, int *last);
/* Applies a received ACKNACK: acknowledges (drops) everything before
 * first_unacked and every clear bit in [first_unacked, first_unacked+16);
 * keeps set (still-missing) bits for retransmit. */
void zdw_reliable_recv_acknack(zdw_reliable *r, int first_unacked,
                               unsigned short nack_bitmap);

/* -- receiver side -- */
/* Buffers an incoming sample. Duplicates (seq < expected, or already buffered)
 * are dropped and return ZDW_REL_OK; ZDW_REL_BUF_FULL when the reorder buffer is
 * full; ZDW_REL_TOO_LARGE if len > ZDW_REL_SLOT_CAP. */
int zdw_reliable_recv_data(zdw_reliable *r, unsigned short seq,
                           const unsigned char *payload, size_t len);
/* Pops the next in-order sample (starting at expected_seq) into out/cap and
 * advances expected_seq; returns 1 with seq/len set, or 0 when the next
 * expected sample is not yet buffered. Call in a loop. */
int zdw_reliable_drain(zdw_reliable *r, unsigned char *out, size_t cap,
                       unsigned short *seq, size_t *len);
/* NACK bitmap for the window [expected, expected+16): bit i set = expected+i not
 * yet received (hint=None semantics -- a clear bit means "received"). */
unsigned short zdw_reliable_pending_acknack(const zdw_reliable *r);
/* Builds the ACKNACK frame for the current receive window (first unacked =
 * expected_seq, bitmap of zdw_reliable_pending_acknack, the stream id of `r`)
 * in the form of the stream's unannotated-types setting. Returns the frame
 * length (17 or 13), or 0 if it does not fit. */
size_t zdw_reliable_acknack_frame(const zdw_reliable *r,
                                  unsigned char *out, size_t cap,
                                  unsigned char session, unsigned char stream,
                                  unsigned int seq);
size_t zdw_reliable_out_of_order_count(const zdw_reliable *r);
unsigned short zdw_reliable_expected(const zdw_reliable *r);

#ifdef __cplusplus
}
#endif

#endif /* ZERODDS_ENDPOINT_H */
