/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors */

#include <string.h>
#include <time.h>

#include "zerodds_xrce_client.h"

/* ------------------------------------------------------------------ */
/* state machine (mirrors crates/xrce-client::XrceClient)             */
/* ------------------------------------------------------------------ */

void zdwc_client_init(zdwc_client *c, const unsigned char client_key[4])
{
    memcpy(c->client_key, client_key, 4);
    c->state = ZDWC_DISCONNECTED;
    c->session_id = 0;
    c->next_request_id = 1;
    c->representation_timestamps = ZDWC_REPRESENTATION_TIMESTAMPS_ABSENT;
    c->unannotated_types = ZDWC_UNANNOTATED_APPENDABLE;
}

void zdwc_set_representation_timestamps(zdwc_client *c, int representation_timestamps,
                                        int unannotated_types)
{
    c->representation_timestamps = representation_timestamps;
    c->unannotated_types = unannotated_types;
}

int zdwc_state(const zdwc_client *c)
{
    return c->state;
}

int zdwc_connect(zdwc_client *c, unsigned char requested_session_id)
{
    if (c->state != ZDWC_DISCONNECTED) {
        return ZDWC_E_INVALID_STATE;
    }
    c->session_id = requested_session_id;
    c->state = ZDWC_CONNECTING;
    return ZDWC_OK;
}

int zdwc_mark_connected(zdwc_client *c)
{
    if (c->state != ZDWC_CONNECTING) {
        return ZDWC_E_INVALID_STATE;
    }
    c->state = ZDWC_CONNECTED;
    return ZDWC_OK;
}

/* Next request_id: 1..65535, wrapping without ever landing on 0 -- the
 * same `wrapping_add(1).max(1)` rule the Rust state machine applies. */
static unsigned int zdwc_next_id(zdwc_client *c)
{
    unsigned int req = c->next_request_id;
    unsigned int nxt = (c->next_request_id + 1) & 0xFFFFu;
    if (nxt == 0u) {
        nxt = 1u;
    }
    c->next_request_id = nxt;
    return req;
}

static int zdwc_require_connected(const zdwc_client *c)
{
    if (c->state != ZDWC_CONNECTED) {
        return ZDWC_E_INVALID_STATE;
    }
    return ZDWC_OK;
}

int zdwc_create_object(zdwc_client *c, unsigned int *out_request_id)
{
    int st = zdwc_require_connected(c);
    if (st != ZDWC_OK) {
        return st;
    }
    *out_request_id = zdwc_next_id(c);
    return ZDWC_OK;
}

int zdwc_delete_object(zdwc_client *c, unsigned int *out_request_id)
{
    int st = zdwc_require_connected(c);
    if (st != ZDWC_OK) {
        return st;
    }
    *out_request_id = zdwc_next_id(c);
    return ZDWC_OK;
}

int zdwc_request_write(zdwc_client *c, unsigned int *out_request_id)
{
    int st = zdwc_require_connected(c);
    if (st != ZDWC_OK) {
        return st;
    }
    *out_request_id = zdwc_next_id(c);
    return ZDWC_OK;
}

int zdwc_request_read(zdwc_client *c, unsigned int *out_request_id)
{
    int st = zdwc_require_connected(c);
    if (st != ZDWC_OK) {
        return st;
    }
    *out_request_id = zdwc_next_id(c);
    return ZDWC_OK;
}

void zdwc_disconnect(zdwc_client *c)
{
    c->state = ZDWC_DISCONNECTED;
}

/* ------------------------------------------------------------------ */
/* framing                                                             */
/* ------------------------------------------------------------------ */

#define XRCE_SM_CREATE_CLIENT 0x00
#define XRCE_SM_CREATE        0x01
#define XRCE_SM_DELETE        0x03
#define XRCE_SM_WRITE_DATA    0x07
#define XRCE_SM_READ_DATA     0x08

#define XRCE_STREAM_NONE             0x00u
#define XRCE_STREAM_BUILTIN_RELIABLE 0x80u

/* request_id (2 octets, spec §7.7.8) as this SDK's own choice of byte
 * order for a host-side monotonic counter: big-endian, matching ObjectId's
 * own wire order (object_id.rs::to_bytes) and the spec's own Annex B
 * examples ("request_id = {0xAA, 0x01}"). The wire itself treats
 * request_id as two opaque octets -- crates/xrce's BaseObjectRequest never
 * interprets it numerically. */
static void put_request_id(unsigned char *out, unsigned int request_id)
{
    out[0] = (unsigned char)((request_id >> 8) & 0xFFu);
    out[1] = (unsigned char)(request_id & 0xFFu);
}

/* ObjectId (2 octets, big-endian -- object_id.rs::to_bytes). */
static void put_object_id(unsigned char *out, unsigned int object_id)
{
    out[0] = (unsigned char)((object_id >> 8) & 0xFFu);
    out[1] = (unsigned char)(object_id & 0xFFu);
}

/* MessageHeader WITH ClientKey (session_id <= 127, §8.3.2.1): 8 bytes.
 * Returns the number of bytes written (always 8) or 0 if it does not fit. */
static size_t put_header_with_key(unsigned char *out, size_t cap,
                                  unsigned char session_id,
                                  unsigned char stream_id,
                                  unsigned int seq,
                                  const unsigned char client_key[4])
{
    if (cap < 8u) {
        return 0;
    }
    out[0] = session_id;
    out[1] = stream_id;
    out[2] = (unsigned char)(seq & 0xFFu);
    out[3] = (unsigned char)((seq >> 8) & 0xFFu);
    out[4] = client_key[0];
    out[5] = client_key[1];
    out[6] = client_key[2];
    out[7] = client_key[3];
    return 8u;
}

/* SubmessageHeader (4 bytes, §8.3.4): id + flags + submessage_length (LE,
 * always -- even under a BE body, spec §8.3.4). Returns 4, or 0 if it does
 * not fit or body_len > 0xFFFF. */
static size_t put_sm_header(unsigned char *out, size_t cap,
                            unsigned char sm_id, unsigned char flags,
                            size_t body_len)
{
    if (cap < 4u || body_len > 0xFFFFu) {
        return 0;
    }
    out[0] = sm_id;
    out[1] = flags;
    out[2] = (unsigned char)(body_len & 0xFFu);
    out[3] = (unsigned char)((body_len >> 8) & 0xFFu);
    return 4u;
}

size_t zdwc_frame_create_client_with(unsigned char session_id,
                                     const unsigned char client_key[4],
                                     int representation_timestamps,
                                     int unannotated_types,
                                     long seconds, unsigned long nanoseconds,
                                     unsigned char *out, size_t cap)
{
    size_t n = 0;
    size_t smh;
    size_t body_len = 16u;
    int with_ts = (representation_timestamps == ZDWC_REPRESENTATION_TIMESTAMPS_PRESENT);
    int with_dheader = (unannotated_types != ZDWC_UNANNOTATED_FINAL);
    unsigned long sec = (unsigned long)seconds;

    if (with_ts) {
        body_len += with_dheader ? 12u : 8u;
    }

    /* DDS-XRCE 1.0 section 8.3.2.1: no session exists yet, so the header carries
     * SESSION_ID_NONE_WITH_CLIENT_KEY (0x00) + ClientKey (8 bytes) when the
     * requested session_id is 0..127, and SESSION_ID_NONE_WITHOUT_CLIENT_KEY
     * (0x80) without ClientKey (4 bytes) when it is 128..255. The requested
     * session_id itself travels in the CLIENT_Representation below. */
    if (session_id < 128u) {
        n = put_header_with_key(out, cap, 0x00u,
                                (unsigned char)XRCE_STREAM_NONE, 0u,
                                client_key);
    } else if (cap >= 4u) {
        out[0] = 0x80u;
        out[1] = (unsigned char)XRCE_STREAM_NONE;
        out[2] = 0u;
        out[3] = 0u;
        n = 4u;
    }
    if (n == 0u) {
        return 0;
    }

    /* Submessage header: body is a real CLIENT_Representation (see the
     * header doc comment): 16 octets, 24 or 28 with the client timestamp. */
    smh = put_sm_header(out + n, cap - n, XRCE_SM_CREATE_CLIENT,
                        0x01 /* E-flag only -- CreateClientPayload::
                              * into_submessage's hardcoded flags */,
                        body_len);
    if (smh == 0u) {
        return 0;
    }
    n += smh;

    /* CLIENT_Representation (Annex A, 7.8.2.1): xrce_cookie(4)
     * + xrce_version(2)={1,0} + xrce_vendor_id(2)={0,0} + [Time_t
     * client_timestamp: DHEADER(4)=8 when appendable, seconds i32,
     * nanoseconds u32, little endian] + client_key(4) + session_id(1) + a `0`
     * properties-absent octet + requested_mtu (2, little endian) =
     * ZDWC_DEFAULT_MTU. Offset 8 is 4-aligned, so no padding. */
    if (cap - n < body_len) {
        return 0;
    }
    out[n++] = 'X'; out[n++] = 'R'; out[n++] = 'C'; out[n++] = 'E';
    out[n++] = 0x01; out[n++] = 0x00; /* version */
    out[n++] = 0x00; out[n++] = 0x00; /* vendor_id */
    if (with_ts) {
        unsigned long ns = nanoseconds;
        if (with_dheader) {
            out[n++] = 0x08; out[n++] = 0x00; out[n++] = 0x00; out[n++] = 0x00;
        }
        out[n++] = (unsigned char)(sec & 0xFFu);
        out[n++] = (unsigned char)((sec >> 8) & 0xFFu);
        out[n++] = (unsigned char)((sec >> 16) & 0xFFu);
        out[n++] = (unsigned char)((sec >> 24) & 0xFFu);
        out[n++] = (unsigned char)(ns & 0xFFu);
        out[n++] = (unsigned char)((ns >> 8) & 0xFFu);
        out[n++] = (unsigned char)((ns >> 16) & 0xFFu);
        out[n++] = (unsigned char)((ns >> 24) & 0xFFu);
    }
    out[n++] = client_key[0];
    out[n++] = client_key[1];
    out[n++] = client_key[2];
    out[n++] = client_key[3];
    out[n++] = session_id;
    out[n++] = 0x00; /* properties absent */
    out[n++] = (unsigned char)(ZDWC_DEFAULT_MTU & 0xFFu);
    out[n++] = (unsigned char)((ZDWC_DEFAULT_MTU >> 8) & 0xFFu);
    return n;
}

size_t zdwc_frame_create_client(unsigned char session_id,
                                const unsigned char client_key[4],
                                unsigned char *out, size_t cap)
{
    return zdwc_frame_create_client_with(session_id, client_key,
                                         ZDWC_REPRESENTATION_TIMESTAMPS_ABSENT,
                                         ZDWC_UNANNOTATED_APPENDABLE, 0L, 0ul,
                                         out, cap);
}

size_t zdwc_client_frame_create_client(const zdwc_client *c, unsigned char session_id,
                                       unsigned char *out, size_t cap)
{
    long now = 0;
    if (c->representation_timestamps == ZDWC_REPRESENTATION_TIMESTAMPS_PRESENT) {
        now = (long)time(NULL);
    }
    return zdwc_frame_create_client_with(session_id, c->client_key,
                                         c->representation_timestamps,
                                         c->unannotated_types, now, 0ul,
                                         out, cap);
}

size_t zdwc_frame_create(unsigned char session_id,
                         const unsigned char client_key[4],
                         unsigned int seq, unsigned int request_id,
                         unsigned int object_id, unsigned int object_kind,
                         const char *reference,
                         int has_parent, unsigned int parent_object_id,
                         int reuse, int replace,
                         unsigned char *out, size_t cap)
{
    size_t n, smh, ref_len, body_len, i;
    unsigned char flags;

    n = put_header_with_key(out, cap, session_id,
                            (unsigned char)XRCE_STREAM_BUILTIN_RELIABLE,
                            seq, client_key);
    if (n == 0u) {
        return 0;
    }

    ref_len = strlen(reference);
    /* BaseObjectRequest(4) + ObjectVariant: kind(1)+disc(1)+pad(2)+
     * len_u32(4)+string bytes+NUL (object_repr.rs::encode_for_kind), plus
     * a trailing parent ObjectId(2, unaligned) when has_parent. */
    body_len = 4u + 1u + 1u + 2u + 4u + ref_len + 1u + (has_parent ? 2u : 0u);

    flags = 0x01u; /* E-flag */
    if (reuse) {
        flags = (unsigned char)(flags | 0x02u);
    }
    if (replace) {
        flags = (unsigned char)(flags | 0x04u);
    }
    smh = put_sm_header(out + n, cap - n, XRCE_SM_CREATE, flags, body_len);
    if (smh == 0u) {
        return 0;
    }
    n += smh;

    if (cap - n < body_len) {
        return 0;
    }
    put_request_id(out + n, request_id);
    n += 2u;
    put_object_id(out + n, object_id);
    n += 2u;
    /* ObjectVariant (§7.7.3, two-level): outer ObjectKind byte, then
     * disc=0x01 (REPRESENTATION_BY_REFERENCE) + 2 bytes padding + u32
     * length (LE, incl. NUL) + string + NUL, then the trailing parent
     * ObjectId when this kind carries one. */
    out[n++] = (unsigned char)(object_kind & 0xFFu);
    out[n++] = 0x01;
    out[n++] = 0; out[n++] = 0;
    {
        unsigned long len_incl_nul = (unsigned long)(ref_len + 1u);
        out[n++] = (unsigned char)(len_incl_nul & 0xFFu);
        out[n++] = (unsigned char)((len_incl_nul >> 8) & 0xFFu);
        out[n++] = (unsigned char)((len_incl_nul >> 16) & 0xFFu);
        out[n++] = (unsigned char)((len_incl_nul >> 24) & 0xFFu);
    }
    for (i = 0; i < ref_len; i++) {
        out[n++] = (unsigned char)reference[i];
    }
    out[n++] = 0; /* NUL terminator */
    if (has_parent) {
        put_object_id(out + n, parent_object_id);
        n += 2u;
    }
    return n;
}

size_t zdwc_frame_delete(unsigned char session_id,
                         const unsigned char client_key[4],
                         unsigned int seq, unsigned int request_id,
                         unsigned int object_id,
                         unsigned char *out, size_t cap)
{
    size_t n, smh;

    n = put_header_with_key(out, cap, session_id,
                            (unsigned char)XRCE_STREAM_BUILTIN_RELIABLE,
                            seq, client_key);
    if (n == 0u) {
        return 0;
    }
    smh = put_sm_header(out + n, cap - n, XRCE_SM_DELETE, 0x01u, 4u);
    if (smh == 0u) {
        return 0;
    }
    n += smh;
    if (cap - n < 4u) {
        return 0;
    }
    put_request_id(out + n, request_id);
    n += 2u;
    put_object_id(out + n, object_id);
    n += 2u;
    return n;
}

size_t zdwc_frame_write_data(unsigned char session_id,
                             const unsigned char client_key[4],
                             unsigned int seq, unsigned int request_id,
                             unsigned int writer_object_id,
                             const unsigned char *sample, size_t sample_len,
                             unsigned char *out, size_t cap)
{
    size_t n, smh, body_len, i;

    if (sample_len > 0xFFFFu - 4u) {
        return 0;
    }
    n = put_header_with_key(out, cap, session_id,
                            (unsigned char)XRCE_STREAM_BUILTIN_RELIABLE,
                            seq, client_key);
    if (n == 0u) {
        return 0;
    }
    body_len = 4u + sample_len;
    smh = put_sm_header(out + n, cap - n, XRCE_SM_WRITE_DATA,
                        0x01u /* E-flag | FORMAT_DATA(0b000)<<1 */, body_len);
    if (smh == 0u) {
        return 0;
    }
    n += smh;
    if (cap - n < body_len) {
        return 0;
    }
    put_request_id(out + n, request_id);
    n += 2u;
    put_object_id(out + n, writer_object_id);
    n += 2u;
    for (i = 0; i < sample_len; i++) {
        out[n++] = sample[i];
    }
    return n;
}

size_t zdwc_frame_read_data(unsigned char session_id,
                            const unsigned char client_key[4],
                            unsigned int seq, unsigned int request_id,
                            unsigned int reader_object_id,
                            unsigned char preferred_stream_id,
                            unsigned char *out, size_t cap)
{
    size_t n, smh;

    n = put_header_with_key(out, cap, session_id,
                            (unsigned char)XRCE_STREAM_BUILTIN_RELIABLE,
                            seq, client_key);
    if (n == 0u) {
        return 0;
    }
    /* BaseObjectRequest(4) + ReadSpecification(4): preferred_stream_id(1) +
     * data_format(1) + content_filter?(1)=0 + delivery_control?(1)=0. */
    smh = put_sm_header(out + n, cap - n, XRCE_SM_READ_DATA, 0x01u, 8u);
    if (smh == 0u) {
        return 0;
    }
    n += smh;
    if (cap - n < 8u) {
        return 0;
    }
    put_request_id(out + n, request_id);
    n += 2u;
    put_object_id(out + n, reader_object_id);
    n += 2u;
    out[n++] = preferred_stream_id;
    out[n++] = 0x00; /* data_format = FORMAT_DATA */
    out[n++] = 0x00; /* content_filter_expression? = absent */
    out[n++] = 0x00; /* delivery_control? = absent */
    return n;
}
