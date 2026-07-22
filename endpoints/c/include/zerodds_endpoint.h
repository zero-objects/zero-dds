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

#ifdef __cplusplus
}
#endif

#endif /* ZERODDS_ENDPOINT_H */
