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

/* WRITE_DATA submessage id (spec §8.3.5.8) + flags = E_LITTLE_ENDIAN(0x01) |
 * DataFormat::Sample(0b001) << 1 = 0x03. */
#define XRCE_SM_WRITE_DATA 0x07
#define XRCE_SM_DATA       0x09  /* agent -> client */
#define XRCE_WRITE_FLAGS   0x03

size_t zdw_xrce_write_frame(unsigned char *out, size_t cap,
                            unsigned char session, unsigned char stream,
                            unsigned int seq, const unsigned char *sample,
                            size_t sample_len)
{
    size_t n = 0;
    /* 4-byte message header + 4-byte submessage header + body. */
    if (8u + sample_len > cap || sample_len > 0xFFFFu) {
        return 0;
    }
    out[n++] = session;
    out[n++] = stream;
    out[n++] = (unsigned char)(seq & 0xffu);        /* sequence_nr LE */
    out[n++] = (unsigned char)((seq >> 8) & 0xffu);
    out[n++] = XRCE_SM_WRITE_DATA;
    out[n++] = XRCE_WRITE_FLAGS;
    out[n++] = (unsigned char)(sample_len & 0xffu); /* submessage_length LE */
    out[n++] = (unsigned char)((sample_len >> 8) & 0xffu);
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
    unsigned int session;
    unsigned int sm_len;
    if (len < 8u) {
        return ZDW_T_ERROR;
    }
    session = frame[0];
    if (session < 0x80u) {
        return ZDW_T_ERROR; /* client-key sessions not handled here */
    }
    /* WRITE_DATA (client->agent, id 7) or DATA (agent->client, id 9): both
     * carry the sample body right after the 8-byte header. */
    if (frame[4] != XRCE_SM_WRITE_DATA && frame[4] != XRCE_SM_DATA) {
        return ZDW_T_ERROR;
    }
    sm_len = (unsigned int)frame[6] | ((unsigned int)frame[7] << 8);
    if ((size_t)sm_len + 8u > len) {
        return ZDW_T_ERROR;
    }
    *body = frame + 8;
    *body_len = (size_t)sm_len;
    return ZDW_T_OK;
}

#define XRCE_FLAG   0x7E
#define XRCE_ESC    0x7D
#define XRCE_STUFF  0x20

unsigned int zdw_crc16_ccitt_false(const unsigned char *data, size_t n)
{
    unsigned int crc = 0xFFFFu;
    size_t i;
    int k;
    for (i = 0; i < n; i++) {
        crc ^= (unsigned int)data[i] << 8;
        for (k = 0; k < 8; k++) {
            if ((crc & 0x8000u) != 0u) {
                crc = ((crc << 1) ^ 0x1021u) & 0xFFFFu;
            } else {
                crc = (crc << 1) & 0xFFFFu;
            }
        }
    }
    return crc & 0xFFFFu;
}

static size_t stuff_into(unsigned char *out, size_t pos, size_t cap,
                         const unsigned char *data, size_t n, int *ok)
{
    size_t i;
    for (i = 0; i < n; i++) {
        unsigned char b = data[i];
        if (b == XRCE_FLAG || b == XRCE_ESC) {
            if (pos + 2u > cap) { *ok = 0; return pos; }
            out[pos++] = XRCE_ESC;
            out[pos++] = (unsigned char)(b ^ XRCE_STUFF);
        } else {
            if (pos + 1u > cap) { *ok = 0; return pos; }
            out[pos++] = b;
        }
    }
    return pos;
}

size_t zdw_serial_frame(unsigned char *out, size_t cap,
                        const unsigned char *payload, size_t n)
{
    unsigned int crc = zdw_crc16_ccitt_false(payload, n);
    unsigned char crcb[2];
    size_t pos = 0;
    int ok = 1;
    crcb[0] = (unsigned char)((crc >> 8) & 0xFFu); /* CRC big-endian */
    crcb[1] = (unsigned char)(crc & 0xFFu);
    if (cap < 1u) { return 0; }
    out[pos++] = XRCE_FLAG;
    pos = stuff_into(out, pos, cap, payload, n, &ok);
    pos = stuff_into(out, pos, cap, crcb, 2, &ok);
    if (!ok || pos + 1u > cap) { return 0; }
    out[pos++] = XRCE_FLAG;
    return pos;
}

int zdw_serial_deframe(const unsigned char *in, size_t len,
                       unsigned char *out, size_t cap, size_t *out_len)
{
    size_t i = 0, o = 0;
    unsigned int crc;
    /* skip a leading flag */
    if (len >= 1u && in[0] == XRCE_FLAG) {
        i = 1;
    }
    while (i < len && in[i] != XRCE_FLAG) {
        unsigned char b = in[i];
        if (b == XRCE_ESC) {
            i++;
            if (i >= len) { return ZDW_T_ERROR; }
            b = (unsigned char)(in[i] ^ XRCE_STUFF);
        }
        if (o >= cap) { return ZDW_T_ERROR; }
        out[o++] = b;
        i++;
    }
    if (o < 2u) { return ZDW_T_ERROR; }
    /* verify + strip the trailing CRC (big-endian) */
    crc = zdw_crc16_ccitt_false(out, o - 2u);
    if (out[o - 2u] != (unsigned char)((crc >> 8) & 0xFFu)
        || out[o - 1u] != (unsigned char)(crc & 0xFFu)) {
        return ZDW_T_ERROR;
    }
    *out_len = o - 2u;
    return ZDW_T_OK;
}

#define XRCE_SM_ACKNACK   0x0A
#define XRCE_SM_HEARTBEAT 0x0B

size_t zdw_xrce_acknack_frame(unsigned char *out, size_t cap,
                              unsigned char session, unsigned char stream,
                              unsigned int seq, int first_unacked,
                              unsigned char nack_lo, unsigned char nack_hi,
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
    out[n++] = XRCE_SM_ACKNACK;
    out[n++] = XRCE_WRITE_FLAGS & 0x01; /* FLAG_E_LITTLE_ENDIAN only (0x01) */
    out[n++] = 5;                       /* submessage_length LE = 5 */
    out[n++] = 0;
    out[n++] = (unsigned char)(first_unacked & 0xff);        /* i16 LE */
    out[n++] = (unsigned char)((first_unacked >> 8) & 0xff);
    out[n++] = nack_lo;
    out[n++] = nack_hi;
    out[n++] = payload_stream;
    return n;
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
    out[n++] = XRCE_WRITE_FLAGS & 0x01; /* FLAG_E_LITTLE_ENDIAN only (0x01) */
    out[n++] = 5;                       /* submessage_length LE = 5 */
    out[n++] = 0;
    out[n++] = (unsigned char)(first & 0xff);        /* i16 LE */
    out[n++] = (unsigned char)((first >> 8) & 0xff);
    out[n++] = (unsigned char)(last & 0xff);         /* i16 LE */
    out[n++] = (unsigned char)((last >> 8) & 0xff);
    out[n++] = payload_stream;
    return n;
}

int zdw_xrce_acknack_read(const unsigned char *frame, size_t len,
                          int *first_unacked, unsigned int *nack,
                          unsigned char *stream)
{
    if (len < 13u) {
        return ZDW_T_ERROR;
    }
    if (frame[4] != XRCE_SM_ACKNACK) {
        return ZDW_T_ERROR;
    }
    *first_unacked = (int)(short)((unsigned int)frame[8] | ((unsigned int)frame[9] << 8));
    *nack = (unsigned int)frame[10] | ((unsigned int)frame[11] << 8);
    *stream = frame[12];
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
    memset(r, 0, sizeof(*r));
    r->stream_id = stream_id;
}

void zdw_reliable_reset(zdw_reliable *r)
{
    unsigned char sid = r->stream_id;
    memset(r, 0, sizeof(*r));
    r->stream_id = sid;
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
    for (i = 0; i < ZDW_REL_WINDOW; i++) {
        if (r->in_flight[i].used) {
            unsigned short s = r->in_flight[i].seq;
            if (!any) {
                mn = s;
                mx = s;
                any = 1;
            } else {
                if (s < mn) { mn = s; }  /* raw ordering, matches BTreeMap keys */
                if (s > mx) { mx = s; }
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
