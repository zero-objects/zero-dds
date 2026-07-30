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

/* Wraps `sample` (an XCDR body) into an XRCE WRITE_DATA (Sample) frame in `out`.
 * Returns the frame length, or 0 if it does not fit. */
size_t zdw_xrce_write_frame(unsigned char *out, size_t cap,
                            unsigned char session, unsigned char stream,
                            unsigned int seq, const unsigned char *sample,
                            size_t sample_len);

/* Locates the sample body inside a received WRITE_DATA frame. On success sets
 * *body / *body_len and returns ZDW_T_OK; ZDW_T_ERROR if the frame is not a
 * best-effort no-key WRITE_DATA. */
int zdw_xrce_read_frame(const unsigned char *frame, size_t len,
                        const unsigned char **body, size_t *body_len);

/* --- XRCE serial framing (DDS-XRCE 1.0 Annex C, RFC 1662 PPP/HDLC) ---
 *
 * Serial links have no frame boundaries, so an XRCE message is wrapped:
 *   7E [ byte-stuffed(payload) byte-stuffed(crc16-BE) ] 7E
 * Stuffing: a 0x7E/0x7D byte becomes 0x7D followed by (byte XOR 0x20). CRC is
 * CRC-16-CCITT-FALSE (init 0xFFFF, poly 0x1021) over the raw payload. This is
 * the framing a `crates/xrce` serial transport exchanges -- the fill for the
 * frame-hook on a serial line. */

/* CRC-16-CCITT-FALSE over `data`. */
unsigned int zdw_crc16_ccitt_false(const unsigned char *data, size_t n);
/* HDLC-frames `payload` into `out`. Returns the frame length, 0 if it does not
 * fit (worst case 2*(n+2)+2 bytes). */
size_t zdw_serial_frame(unsigned char *out, size_t cap,
                        const unsigned char *payload, size_t n);
/* De-frames one HDLC frame (`in` including the 0x7E flags): destuffs, checks
 * the trailing CRC, writes the payload to `out`. On ZDW_T_OK *out_len is set. */
int zdw_serial_deframe(const unsigned char *in, size_t len,
                       unsigned char *out, size_t cap, size_t *out_len);

/* --- reliable stream (DDS-XRCE HEARTBEAT / ACKNACK) ---
 *
 * On a reliable stream (stream_id >= 128) the agent periodically sends a
 * HEARTBEAT (first/last unacked sequence) and the endpoint replies with an
 * ACKNACK (first-unacked + a 16-bit NACK bitmap of missing samples). Both are
 * 5-byte LE bodies. */

/* Builds an ACKNACK message. `nack` is the little-endian 16-bit bitmap (0 = all
 * received). Returns the frame length, or 0 if it does not fit. */
size_t zdw_xrce_acknack_frame(unsigned char *out, size_t cap,
                              unsigned char session, unsigned char stream,
                              unsigned int seq, int first_unacked,
                              unsigned char nack_lo, unsigned char nack_hi,
                              unsigned char payload_stream);
/* Parses a HEARTBEAT frame: *first / *last unacked sequence, *stream. */
int zdw_xrce_heartbeat_read(const unsigned char *frame, size_t len,
                            int *first, int *last, unsigned char *stream);

/* Builds a HEARTBEAT message (the reliable sender announces its unacked window
 * to the peer). Byte-identical to `zdw_xrce_acknack_frame`'s layout with id
 * 0x0B and a 5-byte body (first i16 LE, last i16 LE, stream). Returns the frame
 * length, or 0 if it does not fit. */
size_t zdw_xrce_heartbeat_frame(unsigned char *out, size_t cap,
                                unsigned char session, unsigned char stream,
                                unsigned int seq, int first, int last,
                                unsigned char payload_stream);
/* Parses an ACKNACK frame: *first_unacked, *nack (16-bit LE bitmap), *stream. */
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
} zdw_reliable;

/* Initialise / reset the whole state. */
void zdw_reliable_init(zdw_reliable *r, unsigned char stream_id);
void zdw_reliable_reset(zdw_reliable *r);

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
size_t zdw_reliable_out_of_order_count(const zdw_reliable *r);
unsigned short zdw_reliable_expected(const zdw_reliable *r);

#ifdef __cplusplus
}
#endif

#endif /* ZERODDS_ENDPOINT_H */
