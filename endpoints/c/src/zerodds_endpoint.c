/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors */

#include "zerodds_endpoint.h"

int zdw_endpoint_send(const zdw_transport *t, const unsigned char *frame, size_t len)
{
    if (t == 0 || t->deliver == 0) {
        return ZDW_T_ERROR;
    }
    return t->deliver(t->ctx, frame, len);
}

int zdw_endpoint_recv(const zdw_transport *t, unsigned char *buf, size_t cap, size_t *len)
{
    if (t == 0 || t->receive == 0) {
        return ZDW_T_ERROR;
    }
    return t->receive(t->ctx, buf, cap, len);
}

/* WRITE_DATA submessage id (spec Sec 8.3.5.8). The flag byte is
 * ZDW_XRCE_WRITE_FLAGS = E_LITTLE_ENDIAN(0x01) | FORMAT_DATA(0b000) << 1, and
 * the body is `BaseObjectRequest` (Sec 7.7.8: request_id[2] + object_id[2])
 * followed by the sample. This is what `crates/xrce`'s `WriteDataPayload`
 * writes and reads; a body without the BaseObjectRequest cannot be routed to a
 * DataWriter by any XRCE agent. */
#define XRCE_SM_WRITE_DATA 0x07
#define XRCE_SM_DATA       0x09  /* agent -> client */

/* Locates the submessage body of a WRITE_DATA/DATA frame. Returns ZDW_T_OK and
 * sets *at / *sm_len (body offset and length) on success. */
static int zdw_xrce_locate_body(const unsigned char *frame, size_t len,
                                size_t *at, size_t *sm_len)
{
    unsigned int declared;
    if (len < 8u) {
        return ZDW_T_ERROR;
    }
    if (frame[0] < 0x80u) {
        return ZDW_T_ERROR; /* client-key sessions not handled here */
    }
    /* WRITE_DATA (client->agent, id 7) or DATA (agent->client, id 9). */
    if (frame[4] != XRCE_SM_WRITE_DATA && frame[4] != XRCE_SM_DATA) {
        return ZDW_T_ERROR;
    }
    declared = (unsigned int)frame[6] | ((unsigned int)frame[7] << 8);
    if ((size_t)declared + 8u > len) {
        return ZDW_T_ERROR;
    }
    /* A body shorter than the BaseObjectRequest is not a data submessage;
     * refuse it loudly rather than handing back a truncated sample. */
    if ((size_t)declared < ZDW_XRCE_BASE_SIZE) {
        return ZDW_T_ERROR;
    }
    *at = 8u;
    *sm_len = (size_t)declared;
    return ZDW_T_OK;
}

size_t zdw_xrce_write_frame(unsigned char *out, size_t cap,
                            unsigned char session, unsigned char stream,
                            unsigned int seq, unsigned int object_id,
                            const unsigned char *sample,
                            size_t sample_len)
{
    size_t n = 0;
    size_t body_len;
    if (object_id > 0xFFFFu) {
        return 0;
    }
    body_len = ZDW_XRCE_BASE_SIZE + sample_len;
    /* 4-byte message header + 4-byte submessage header + body. */
    if (8u + body_len > cap || body_len > 0xFFFFu) {
        return 0;
    }
    out[n++] = session;
    out[n++] = stream;
    out[n++] = (unsigned char)(seq & 0xffu);        /* sequence_nr LE */
    out[n++] = (unsigned char)((seq >> 8) & 0xffu);
    out[n++] = XRCE_SM_WRITE_DATA;
    out[n++] = ZDW_XRCE_WRITE_FLAGS;
    out[n++] = (unsigned char)(body_len & 0xffu);   /* submessage_length LE */
    out[n++] = (unsigned char)((body_len >> 8) & 0xffu);
    /* BaseObjectRequest: RequestId(2) + ObjectId(2), most significant octet
     * first (Sec 7.7.8 / Sec 7.2.1). The RequestId is the message sequence
     * number -- unique per stream, which is all Sec 7.7.8 requires. */
    out[n++] = (unsigned char)((seq >> 8) & 0xffu);
    out[n++] = (unsigned char)(seq & 0xffu);
    out[n++] = (unsigned char)((object_id >> 8) & 0xffu);
    out[n++] = (unsigned char)(object_id & 0xffu);
    {
        size_t i;
        for (i = 0; i < sample_len; i++) {
            out[n++] = sample[i];
        }
    }
    return n;
}

int zdw_xrce_read_frame(const unsigned char *frame, size_t len,
                        const unsigned char **body, size_t *body_len)
{
    size_t at;
    size_t sm_len;
    if (zdw_xrce_locate_body(frame, len, &at, &sm_len) != ZDW_T_OK) {
        return ZDW_T_ERROR;
    }
    *body = frame + at + ZDW_XRCE_BASE_SIZE;
    *body_len = sm_len - ZDW_XRCE_BASE_SIZE;
    return ZDW_T_OK;
}

int zdw_xrce_frame_base(const unsigned char *frame, size_t len,
                        unsigned int *request_id, unsigned int *object_id)
{
    size_t at;
    size_t sm_len;
    if (zdw_xrce_locate_body(frame, len, &at, &sm_len) != ZDW_T_OK) {
        return ZDW_T_ERROR;
    }
    if (request_id != 0) {
        *request_id = ((unsigned int)frame[at] << 8) | (unsigned int)frame[at + 1u];
    }
    if (object_id != 0) {
        *object_id = ((unsigned int)frame[at + 2u] << 8) | (unsigned int)frame[at + 3u];
    }
    return ZDW_T_OK;
}

#define XRCE_FLAG   0x7E
#define XRCE_ESC    0x7D
#define XRCE_STUFF  0x20

/* Running FCS-16 of RFC 1662 over one more byte (reflected 0x8408). */
static unsigned int fcs16_step(unsigned int crc, unsigned char b)
{
    int k;
    crc ^= (unsigned int)b;
    for (k = 0; k < 8; k++) {
        crc = ((crc & 0x0001u) != 0u) ? ((crc >> 1) ^ 0x8408u) : (crc >> 1);
    }
    return crc;
}

unsigned int zdw_fcs16(const unsigned char *data, size_t n)
{
    unsigned int crc = 0xFFFFu;
    size_t i;
    for (i = 0; i < n; i++) {
        crc = fcs16_step(crc, data[i]);
    }
    return (~crc) & 0xFFFFu;
}

/* Appends `b` stuffed (Annex C.1.2); 0 if it does not fit. */
static int stuff_byte(unsigned char *out, size_t *pos, size_t cap, unsigned char b)
{
    if (b == XRCE_FLAG || b == XRCE_ESC) {
        if (*pos + 2u > cap) { return 0; }
        out[(*pos)++] = XRCE_ESC;
        out[(*pos)++] = (unsigned char)(b ^ XRCE_STUFF);
    } else {
        if (*pos + 1u > cap) { return 0; }
        out[(*pos)++] = b;
    }
    return 1;
}

size_t zdw_serial_frame(unsigned char *out, size_t cap,
                        unsigned char source, unsigned char remote,
                        const unsigned char *payload, size_t n)
{
    unsigned char head[ZDW_SERIAL_HEADER_LEN];
    unsigned int crc = 0xFFFFu;
    size_t pos = 0, i;
    if (n > 0xFFFFu || cap < 1u) { return 0; }
    head[0] = source;
    head[1] = remote;
    head[2] = (unsigned char)(n & 0xFFu);        /* payloadLength LE */
    head[3] = (unsigned char)((n >> 8) & 0xFFu);
    out[pos++] = XRCE_FLAG;
    for (i = 0; i < ZDW_SERIAL_HEADER_LEN; i++) {
        crc = fcs16_step(crc, head[i]);
        if (!stuff_byte(out, &pos, cap, head[i])) { return 0; }
    }
    for (i = 0; i < n; i++) {
        crc = fcs16_step(crc, payload[i]);
        if (!stuff_byte(out, &pos, cap, payload[i])) { return 0; }
    }
    crc = (~crc) & 0xFFFFu;
    if (!stuff_byte(out, &pos, cap, (unsigned char)(crc & 0xFFu))      /* frameCheck LE */
        || !stuff_byte(out, &pos, cap, (unsigned char)((crc >> 8) & 0xFFu))) {
        return 0;
    }
    return pos;
}

int zdw_serial_deframe(const unsigned char *in, size_t len,
                       unsigned char *source, unsigned char *remote,
                       unsigned char *out, size_t cap, size_t *out_len,
                       size_t *consumed)
{
    unsigned char head[ZDW_SERIAL_HEADER_LEN] = {0, 0, 0, 0}, check[2] = {0, 0};
    unsigned int crc = 0xFFFFu;
    size_t i, got = 0, plen = 0, total = ZDW_SERIAL_HEADER_LEN + 2u;
    if (len < 1u || in[0] != XRCE_FLAG) { return ZDW_T_ERROR; }
    for (i = 1; i < len && got < total; i++) {
        unsigned char b = in[i];
        if (b == XRCE_FLAG) { return ZDW_T_ERROR; } /* C.1.3 */
        if (b == XRCE_ESC) {
            i++;
            if (i >= len) { return ZDW_T_ERROR; }
            b = (unsigned char)(in[i] ^ XRCE_STUFF);
            if (b != XRCE_FLAG && b != XRCE_ESC) { return ZDW_T_ERROR; }
        }
        if (got < ZDW_SERIAL_HEADER_LEN) {
            head[got] = b;
            crc = fcs16_step(crc, b);
            if (got + 1u == ZDW_SERIAL_HEADER_LEN) {
                plen = (size_t)head[2] | ((size_t)head[3] << 8);
                if (plen > cap) { return ZDW_T_ERROR; }
                total = ZDW_SERIAL_HEADER_LEN + plen + 2u;
            }
        } else if (got < ZDW_SERIAL_HEADER_LEN + plen) {
            out[got - ZDW_SERIAL_HEADER_LEN] = b;
            crc = fcs16_step(crc, b);
        } else {
            check[got - ZDW_SERIAL_HEADER_LEN - plen] = b;
        }
        got++;
    }
    if (got < total) { return ZDW_T_ERROR; }
    crc = (~crc) & 0xFFFFu;
    if (check[0] != (unsigned char)(crc & 0xFFu)
        || check[1] != (unsigned char)((crc >> 8) & 0xFFu)) {
        return ZDW_T_ERROR;
    }
    if (source != 0) { *source = head[0]; }
    if (remote != 0) { *remote = head[1]; }
    *out_len = plen;
    if (consumed != 0) { *consumed = i; }
    return ZDW_T_OK;
}

#define XRCE_SM_ACKNACK   0x0A
#define XRCE_SM_HEARTBEAT 0x0B

/* ACKNACK_Payload has no @extensibility in DDS-XRCE 1.0 Annex A, so it is
 * APPENDABLE (DDS-XTypes 1.3 7.3.1.2.1.8) and the XCDR2 body starts with a
 * 4-octet DHEADER holding the member size (5): body = 9 octets, frame = 17.
 * With ZDW_UNANNOTATED_FINAL the DHEADER is left out (body 5, frame 13), the
 * form of the reference implementation. */
size_t zdw_xrce_acknack_frame_with(unsigned char *out, size_t cap,
                                   unsigned char session, unsigned char stream,
                                   unsigned int seq, int first_unacked,
                                   unsigned char nack_lo, unsigned char nack_hi,
                                   unsigned char payload_stream,
                                   int unannotated_types)
{
    size_t n = 0;
    int appendable = (unannotated_types != ZDW_UNANNOTATED_FINAL);
    if (cap < (appendable ? 17u : 13u)) { /* 4 header + 4 submsg header + body */
        return 0;
    }
    out[n++] = session;
    out[n++] = stream;
    out[n++] = (unsigned char)(seq & 0xffu);
    out[n++] = (unsigned char)((seq >> 8) & 0xffu);
    out[n++] = XRCE_SM_ACKNACK;
    out[n++] = ZDW_XRCE_FLAG_E_LE; /* FLAG_E_LITTLE_ENDIAN only (0x01) */
    out[n++] = appendable ? 9 : 5;      /* submessage_length LE */
    out[n++] = 0;
    if (appendable) {
        out[n++] = 5;                   /* DHEADER u32 LE = 5 */
        out[n++] = 0;
        out[n++] = 0;
        out[n++] = 0;
    }
    out[n++] = (unsigned char)(first_unacked & 0xff);        /* i16 LE */
    out[n++] = (unsigned char)((first_unacked >> 8) & 0xff);
    out[n++] = nack_lo;
    out[n++] = nack_hi;
    out[n++] = payload_stream;
    return n;
}

size_t zdw_xrce_acknack_frame(unsigned char *out, size_t cap,
                              unsigned char session, unsigned char stream,
                              unsigned int seq, int first_unacked,
                              unsigned char nack_lo, unsigned char nack_hi,
                              unsigned char payload_stream)
{
    return zdw_xrce_acknack_frame_with(out, cap, session, stream, seq,
                                       first_unacked, nack_lo, nack_hi,
                                       payload_stream,
                                       ZDW_UNANNOTATED_APPENDABLE);
}

int zdw_xrce_heartbeat_read(const unsigned char *frame, size_t len,
                            int *first, int *last, unsigned char *stream)
{
    if (len < 13u) {
        return ZDW_T_ERROR;
    }
    if (frame[4] != XRCE_SM_HEARTBEAT) {
        return ZDW_T_ERROR;
    }
    *first = (int)(short)((unsigned int)frame[8] | ((unsigned int)frame[9] << 8));
    *last = (int)(short)((unsigned int)frame[10] | ((unsigned int)frame[11] << 8));
    *stream = frame[12];
    return ZDW_T_OK;
}

size_t zdw_xrce_heartbeat_frame(unsigned char *out, size_t cap,
                                unsigned char session, unsigned char stream,
                                unsigned int seq, int first, int last,
                                unsigned char payload_stream)
{
    size_t n = 0;
    if (cap < 13u) { /* 4 header + 4 submsg header + 5 body */
        return 0;
    }
    out[n++] = session;
    out[n++] = stream;
    out[n++] = (unsigned char)(seq & 0xffu);
    out[n++] = (unsigned char)((seq >> 8) & 0xffu);
    out[n++] = XRCE_SM_HEARTBEAT;
    out[n++] = ZDW_XRCE_FLAG_E_LE; /* FLAG_E_LITTLE_ENDIAN only (0x01) */
    out[n++] = 5;                       /* submessage_length LE = 5 */
    out[n++] = 0;
    out[n++] = (unsigned char)(first & 0xff);        /* i16 LE */
    out[n++] = (unsigned char)((first >> 8) & 0xff);
    out[n++] = (unsigned char)(last & 0xff);         /* i16 LE */
    out[n++] = (unsigned char)((last >> 8) & 0xff);
    out[n++] = payload_stream;
    return n;
}

/* Accepts the XCDR2 form (submessage_length 9, 4-octet DHEADER first) and the
 * legacy FINAL form (submessage_length 5, no DHEADER) sent by eProsima clients. */
int zdw_xrce_acknack_read(const unsigned char *frame, size_t len,
                          int *first_unacked, unsigned int *nack,
                          unsigned char *stream)
{
    size_t body_len;
    size_t off;
    if (len < 13u) {
        return ZDW_T_ERROR;
    }
    if (frame[4] != XRCE_SM_ACKNACK) {
        return ZDW_T_ERROR;
    }
    body_len = (size_t)frame[6] | ((size_t)frame[7] << 8);
    if (body_len >= 9u) {
        off = 12u; /* skip the 4-octet DHEADER */
    } else if (body_len == 5u) {
        off = 8u;
    } else {
        return ZDW_T_ERROR;
    }
    if (len < off + 5u) {
        return ZDW_T_ERROR;
    }
    *first_unacked = (int)(short)((unsigned int)frame[off] | ((unsigned int)frame[off + 1] << 8));
    *nack = (unsigned int)frame[off + 2] | ((unsigned int)frame[off + 3] << 8);
    *stream = frame[off + 4];
    return ZDW_T_OK;
}

/* ==================================================================
 * Reliable stream state machine (mirrors crates/xrce reliable.rs)
 * ================================================================== */

#include <string.h>

/* RFC-1982 §3.2: is `a` strictly before `b` (mod 2^16)? */
static int zdw_rel_seq_lt(unsigned short a, unsigned short b)
{
    return (short)(unsigned short)(a - b) < 0;
}

void zdw_reliable_init(zdw_reliable *r, unsigned char stream_id)
{
    zdw_reliable_init_with(r, stream_id, ZDW_UNANNOTATED_APPENDABLE);
}

void zdw_reliable_init_with(zdw_reliable *r, unsigned char stream_id,
                            int unannotated_types)
{
    memset(r, 0, sizeof(*r));
    r->stream_id = stream_id;
    r->unannotated_types = (unannotated_types == ZDW_UNANNOTATED_FINAL)
                               ? ZDW_UNANNOTATED_FINAL
                               : ZDW_UNANNOTATED_APPENDABLE;
}

void zdw_reliable_reset(zdw_reliable *r)
{
    unsigned char sid = r->stream_id;
    int mode = r->unannotated_types;
    zdw_reliable_init_with(r, sid, mode);
}

int zdw_reliable_unannotated_types(const zdw_reliable *r)
{
    return r->unannotated_types;
}

size_t zdw_reliable_acknack_frame(const zdw_reliable *r,
                                  unsigned char *out, size_t cap,
                                  unsigned char session, unsigned char stream,
                                  unsigned int seq)
{
    unsigned short bm = zdw_reliable_pending_acknack(r);
    return zdw_xrce_acknack_frame_with(out, cap, session, stream, seq,
                                       (int)r->expected_seq,
                                       (unsigned char)(bm & 0xffu),
                                       (unsigned char)((bm >> 8) & 0xffu),
                                       r->stream_id, r->unannotated_types);
}

int zdw_reliable_submit(zdw_reliable *r, const unsigned char *payload,
                        size_t len, unsigned short *out_seq)
{
    int i, free_idx = -1, used = 0;
    if (len > (size_t)ZDW_REL_SLOT_CAP) {
        return ZDW_REL_TOO_LARGE;
    }
    for (i = 0; i < ZDW_REL_WINDOW; i++) {
        if (r->in_flight[i].used) {
            used++;
        } else if (free_idx < 0) {
            free_idx = i;
        }
    }
    if (used >= ZDW_REL_WINDOW || free_idx < 0) {
        return ZDW_REL_WINDOW_FULL;
    }
    r->in_flight[free_idx].used = 1;
    r->in_flight[free_idx].seq = r->next_seq;
    r->in_flight[free_idx].len = (unsigned short)len;
    if (len > 0u) {
        memcpy(r->in_flight[free_idx].buf, payload, len);
    }
    if (out_seq != 0) {
        *out_seq = r->next_seq;
    }
    r->next_seq = (unsigned short)(r->next_seq + 1);
    return ZDW_REL_OK;
}

size_t zdw_reliable_in_flight_count(const zdw_reliable *r)
{
    size_t c = 0;
    int i;
    for (i = 0; i < ZDW_REL_WINDOW; i++) {
        if (r->in_flight[i].used) {
            c++;
        }
    }
    return c;
}

const unsigned char *zdw_reliable_get_in_flight(const zdw_reliable *r,
                                                unsigned short seq, size_t *len)
{
    int i;
    for (i = 0; i < ZDW_REL_WINDOW; i++) {
        if (r->in_flight[i].used && r->in_flight[i].seq == seq) {
            if (len != 0) {
                *len = r->in_flight[i].len;
            }
            return r->in_flight[i].buf;
        }
    }
    return 0;
}

int zdw_reliable_pending_heartbeat(zdw_reliable *r, unsigned long now_ms,
                                   int *first, int *last)
{
    int i, any = 0, due;
    unsigned short mn = 0, mx = 0;
    /* RFC-1982 window base (oldest unacked) + end (newest unacked); NOT the
     * numeric min/max, which is wrong across a 16-bit wrap: window
     * 0xFFFE,0xFFFF,0x0000,0x0001 -> base 0xFFFE / end 0x0001, not 0x0000 /
     * 0xFFFF. Mirrors window_base / serial_max_in_flight in crates/xrce. */
    for (i = 0; i < ZDW_REL_WINDOW; i++) {
        if (r->in_flight[i].used) {
            unsigned short s = r->in_flight[i].seq;
            if (!any) {
                mn = s;
                mx = s;
                any = 1;
            } else {
                if (zdw_rel_seq_lt(s, mn)) { mn = s; }
                if (zdw_rel_seq_lt(mx, s)) { mx = s; }
            }
        }
    }
    if (!any) {
        return 0;
    }
    due = (!r->hb_started) || ((now_ms - r->last_hb_ms) >= (unsigned long)ZDW_REL_HEARTBEAT_MS);
    if (!due) {
        return 0;
    }
    r->hb_started = 1;
    r->last_hb_ms = now_ms;
    if (first != 0) { *first = (int)mn; }
    if (last != 0) { *last = (int)mx; }
    return 1;
}

void zdw_reliable_recv_acknack(zdw_reliable *r, int first_unacked,
                               unsigned short nack_bitmap)
{
    unsigned short base = (unsigned short)first_unacked;
    unsigned short bit;
    int i;
    /* 1) acknowledge (drop) everything strictly before base. */
    for (i = 0; i < ZDW_REL_WINDOW; i++) {
        if (r->in_flight[i].used && zdw_rel_seq_lt(r->in_flight[i].seq, base)) {
            r->in_flight[i].used = 0;
        }
    }
    /* 2) within [base, base+16): a clear bit means acknowledged -> drop. */
    for (bit = 0; bit < 16; bit++) {
        if (((nack_bitmap >> bit) & 1u) == 0u) {
            unsigned short seq = (unsigned short)(base + bit);
            for (i = 0; i < ZDW_REL_WINDOW; i++) {
                if (r->in_flight[i].used && r->in_flight[i].seq == seq) {
                    r->in_flight[i].used = 0;
                }
            }
        }
    }
}

int zdw_reliable_recv_data(zdw_reliable *r, unsigned short seq,
                           const unsigned char *payload, size_t len)
{
    int i, free_idx = -1, used = 0;
    if (len > (size_t)ZDW_REL_SLOT_CAP) {
        return ZDW_REL_TOO_LARGE;
    }
    if (zdw_rel_seq_lt(seq, r->expected_seq)) {
        return ZDW_REL_OK; /* already delivered -> duplicate, drop */
    }
    for (i = 0; i < ZDW_REL_RECV_BUF; i++) {
        if (r->recv_buf[i].used) {
            used++;
            if (r->recv_buf[i].seq == seq) {
                return ZDW_REL_OK; /* already buffered */
            }
        } else if (free_idx < 0) {
            free_idx = i;
        }
    }
    if (used >= ZDW_REL_RECV_BUF || free_idx < 0) {
        return ZDW_REL_BUF_FULL;
    }
    r->recv_buf[free_idx].used = 1;
    r->recv_buf[free_idx].seq = seq;
    r->recv_buf[free_idx].len = (unsigned short)len;
    if (len > 0u) {
        memcpy(r->recv_buf[free_idx].buf, payload, len);
    }
    return ZDW_REL_OK;
}

int zdw_reliable_drain(zdw_reliable *r, unsigned char *out, size_t cap,
                       unsigned short *seq, size_t *len)
{
    int i;
    for (i = 0; i < ZDW_REL_RECV_BUF; i++) {
        if (r->recv_buf[i].used && r->recv_buf[i].seq == r->expected_seq) {
            size_t l = r->recv_buf[i].len;
            if (l > cap) {
                return 0; /* caller buffer too small */
            }
            if (l > 0u) {
                memcpy(out, r->recv_buf[i].buf, l);
            }
            if (seq != 0) { *seq = r->expected_seq; }
            if (len != 0) { *len = l; }
            r->recv_buf[i].used = 0;
            r->expected_seq = (unsigned short)(r->expected_seq + 1);
            return 1;
        }
    }
    return 0;
}

unsigned short zdw_reliable_pending_acknack(const zdw_reliable *r)
{
    unsigned short bitmap = 0, bit;
    int i;
    for (bit = 0; bit < 16; bit++) {
        unsigned short seq = (unsigned short)(r->expected_seq + bit);
        int found = 0;
        for (i = 0; i < ZDW_REL_RECV_BUF; i++) {
            if (r->recv_buf[i].used && r->recv_buf[i].seq == seq) {
                found = 1;
                break;
            }
        }
        if (!found) {
            bitmap = (unsigned short)(bitmap | (unsigned short)(1u << bit));
        }
    }
    return bitmap;
}

size_t zdw_reliable_out_of_order_count(const zdw_reliable *r)
{
    size_t c = 0;
    int i;
    for (i = 0; i < ZDW_REL_RECV_BUF; i++) {
        if (r->recv_buf[i].used) {
            c++;
        }
    }
    return c;
}

unsigned short zdw_reliable_expected(const zdw_reliable *r)
{
    return r->expected_seq;
}
