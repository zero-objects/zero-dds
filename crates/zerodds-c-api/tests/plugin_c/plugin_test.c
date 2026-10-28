/* SPDX-License-Identifier: Apache-2.0 */
/* Copyright 2026 ZeroDDS Contributors */

/*
 * A DDS-Security 1.2 Authentication, AccessControl, Cryptographic, Logging and
 * DataTagging plugin written in C and placed into a participant.
 *
 *     plugin_test <domain> <identity_ca> <identity_cert> <identity_key>
 *                 <permissions_ca> <governance.p7s> <permissions.p7s>
 *
 * The AccessControl plugin refuses the DataWriter and the DataReader of the
 * topic "Forbidden" and permits every other operation. The Logging plugin
 * counts what the runtime logs; the DataTagging plugin gives every endpoint
 * the tag owner=c-plugin. The program checks that the runtime called each
 * plugin where the specification calls it and that it destroyed each plugin
 * when it let go of it; it prints "OK" and returns 0, or names the first
 * check that failed.
 */

#define _DEFAULT_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "zerodds.h"
#include "zerodds_security_plugins.h"

#define FORBIDDEN "Forbidden"

/* The sizes the Rust side asserts for the same structs
 * (plugin_ffi/security_plugins.rs, the_structs_have_the_layout_of_the_header). */
#define SIZE_IS(type, n) typedef char size_of_##type[(sizeof(type) == (n)) ? 1 : -1]
SIZE_IS(ZeroDdsPluginBytes, 16);
SIZE_IS(ZeroDdsPluginHandshakeStep, 32);
SIZE_IS(ZeroDdsPluginAuthenticationListener, 24);
SIZE_IS(ZeroDdsPluginEndpointAlgorithmInfo, 8);
SIZE_IS(ZeroDdsPluginCryptoTokens, 24);
SIZE_IS(ZeroDdsPluginEncodedPayload, 40);
SIZE_IS(ZeroDdsPluginAuthentication, 16 + 8 * 23);
SIZE_IS(ZeroDdsPluginCryptographic, 16 + 8 * 28);

/* ------------------------------------------------------------------ state */

static struct {
    int validate_local;
    int create_participant;
    int create_writer;
    int create_reader;
    int refused_writer;
    int refused_reader;
    int permissions_token_out;
    int permissions_token_back;
    int credential_token_out;
    int credential_token_back;
    int participant_config_out;
    int participant_config_back;
    int writer_config_out;
    int writer_config_back;
    int reader_config_out;
    int reader_config_back;
    int topic_config_back;
    int listener_set;
    int destroyed;
} ac;

static struct {
    int options;
    int enabled;
    int messages;
    int denied;
    int destroyed;
} lg;

static struct {
    int set_tags;
    int get_tags;
    int destroyed;
    uint8_t guid[16];
} tg;

static struct {
    int validate_local;
    int has_governance_property;
    int identity_token_out, identity_token_back;
    int status_token_out, status_token_back;
    int credential_set;
    int config_set;
    int credential_before_config;
    int listener_set;
    int destroyed;
} au;

static struct {
    int register_participant;
    int participant_axk;
    int register_datawriter, register_datareader;
    int volatile_writer_named, volatile_reader_named;
    int protected_writer_registered;
    int encode_payload;
    int destroyed;
} cr;

/* ------------------------------------------------------------ AccessControl */

static const ZeroDdsPluginProperty owner_property = {"owner", "c-plugin", false};
static const ZeroDdsPluginDataHolder permissions_token = {
    "DDS:Access:Permissions:C", &owner_property, 1, NULL, 0};
static const ZeroDdsPluginDataHolder credential_token = {
    "DDS:Access:Credential:C", &owner_property, 1, NULL, 0};

static const char *ac_class_id(void *user) {
    (void)user;
    return "DDS:Access:C";
}

static ZeroDdsPluginStatus ac_validate_local(void *user, uint64_t identity,
        const ZeroDdsPluginParticipantRequest *p, uint64_t *out_handle,
        ZeroDdsPluginError *err) {
    (void)user; (void)identity; (void)p; (void)err;
    ac.validate_local++;
    *out_handle = 77;
    return 0;
}

static ZeroDdsPluginStatus ac_validate_remote(void *user, uint64_t li, uint64_t ri,
        const ZeroDdsPluginDataHolder *pt, const ZeroDdsPluginDataHolder *ct,
        uint64_t *out_handle, ZeroDdsPluginError *err) {
    (void)user; (void)li; (void)ri; (void)pt; (void)ct; (void)err;
    *out_handle = 78;
    return 0;
}

#define PERMIT_PARTICIPANT(name) \
    static ZeroDdsPluginStatus name(void *user, uint64_t permissions, \
            const ZeroDdsPluginParticipantRequest *p, bool *out_permit, \
            ZeroDdsPluginError *err) { \
        (void)user; (void)permissions; (void)p; (void)err; \
        *out_permit = true; \
        return 0; \
    }
PERMIT_PARTICIPANT(ac_check_remote_participant)

static ZeroDdsPluginStatus ac_check_create_participant(void *user, uint64_t permissions,
        const ZeroDdsPluginParticipantRequest *p, bool *out_permit,
        ZeroDdsPluginError *err) {
    (void)user; (void)err;
    /* The handle validate_local_permissions gave; the domain of the request. */
    ac.create_participant += (permissions == 77 && p->domain_id < 233) ? 1 : 0;
    *out_permit = true;
    return 0;
}

#define PERMIT_ENDPOINT(name) \
    static ZeroDdsPluginStatus name(void *user, uint64_t permissions, \
            const ZeroDdsPluginEndpointRequest *r, bool *out_permit, \
            ZeroDdsPluginError *err) { \
        (void)user; (void)permissions; (void)r; (void)err; \
        *out_permit = true; \
        return 0; \
    }
PERMIT_ENDPOINT(ac_check_remote_datawriter)

static ZeroDdsPluginStatus ac_check_create_datawriter(void *user, uint64_t permissions,
        const ZeroDdsPluginEndpointRequest *r, bool *out_permit,
        ZeroDdsPluginError *err) {
    (void)user; (void)permissions; (void)err;
    ac.create_writer++;
    *out_permit = strcmp(r->topic_name, FORBIDDEN) != 0;
    if (!*out_permit) ac.refused_writer++;
    return 0;
}

static ZeroDdsPluginStatus ac_check_create_datareader(void *user, uint64_t permissions,
        const ZeroDdsPluginEndpointRequest *r, bool *out_permit,
        ZeroDdsPluginError *err) {
    (void)user; (void)permissions; (void)err;
    ac.create_reader++;
    *out_permit = strcmp(r->topic_name, FORBIDDEN) != 0;
    if (!*out_permit) ac.refused_reader++;
    return 0;
}

static ZeroDdsPluginStatus ac_check_remote_datareader(void *user, uint64_t permissions,
        const ZeroDdsPluginEndpointRequest *r, int32_t *out_decision,
        ZeroDdsPluginError *err) {
    (void)user; (void)permissions; (void)r; (void)err;
    *out_decision = ZERODDS_PLUGIN_PERMIT;
    return 0;
}

#define PERMIT_TOPIC(name) \
    static ZeroDdsPluginStatus name(void *user, uint64_t permissions, \
            const ZeroDdsPluginTopicRequest *r, bool *out_permit, \
            ZeroDdsPluginError *err) { \
        (void)user; (void)permissions; (void)r; (void)err; \
        *out_permit = true; \
        return 0; \
    }
PERMIT_TOPIC(ac_check_create_topic)
PERMIT_TOPIC(ac_check_remote_topic)

#define PERMIT_INSTANCE(name) \
    static ZeroDdsPluginStatus name(void *user, uint64_t permissions, \
            const uint8_t writer[16], const uint8_t *key, size_t key_len, \
            bool *out_permit, ZeroDdsPluginError *err) { \
        (void)user; (void)permissions; (void)writer; (void)key; (void)key_len; \
        (void)err; \
        *out_permit = true; \
        return 0; \
    }
PERMIT_INSTANCE(ac_check_local_register)
PERMIT_INSTANCE(ac_check_local_dispose)

#define PERMIT_REMOTE_INSTANCE(name) \
    static ZeroDdsPluginStatus name(void *user, uint64_t permissions, \
            const uint8_t reader[16], const uint8_t publication[16], \
            const uint8_t *key, size_t key_len, bool *out_permit, \
            ZeroDdsPluginError *err) { \
        (void)user; (void)permissions; (void)reader; (void)publication; \
        (void)key; (void)key_len; (void)err; \
        *out_permit = true; \
        return 0; \
    }
PERMIT_REMOTE_INSTANCE(ac_check_remote_register)
PERMIT_REMOTE_INSTANCE(ac_check_remote_dispose)

#define PERMIT_MATCH(name) \
    static ZeroDdsPluginStatus name(void *user, uint64_t wp, uint64_t rp, \
            const ZeroDdsPluginEndpointRequest *a, \
            const ZeroDdsPluginEndpointRequest *b, bool *out_permit, \
            ZeroDdsPluginError *err) { \
        (void)user; (void)wp; (void)rp; (void)a; (void)b; (void)err; \
        *out_permit = true; \
        return 0; \
    }
PERMIT_MATCH(ac_check_local_datawriter_match)
PERMIT_MATCH(ac_check_local_datareader_match)

static ZeroDdsPluginStatus ac_check_relay(void *user, uint64_t permissions, int32_t role,
        const ZeroDdsPluginEndpointRequest *r, bool *out_permit,
        ZeroDdsPluginError *err) {
    (void)user; (void)permissions; (void)role; (void)r; (void)err;
    *out_permit = false;
    return 0;
}

static ZeroDdsPluginStatus ac_get_permissions_token(void *user, uint64_t handle,
        ZeroDdsPluginDataHolder *out, ZeroDdsPluginError *err) {
    (void)user; (void)handle; (void)err;
    ac.permissions_token_out++;
    *out = permissions_token;
    return 0;
}

static ZeroDdsPluginStatus ac_get_credential_token(void *user, uint64_t handle,
        ZeroDdsPluginDataHolder *out, ZeroDdsPluginError *err) {
    (void)user; (void)handle; (void)err;
    ac.credential_token_out++;
    *out = credential_token;
    return 0;
}

static ZeroDdsPluginStatus ac_set_listener(void *user,
        const ZeroDdsPluginAccessControlListener *l, ZeroDdsPluginError *err) {
    (void)user; (void)l; (void)err;
    ac.listener_set++;
    return 0;
}

static ZeroDdsPluginStatus ac_return_permissions_token(void *user,
        const ZeroDdsPluginDataHolder *t, ZeroDdsPluginError *err) {
    (void)user; (void)err;
    if (t->class_id == NULL || strcmp(t->class_id, permissions_token.class_id) != 0)
        return 5;
    ac.permissions_token_back++;
    return 0;
}

static ZeroDdsPluginStatus ac_return_credential_token(void *user,
        const ZeroDdsPluginDataHolder *t, ZeroDdsPluginError *err) {
    (void)user; (void)err;
    if (t->class_id == NULL || strcmp(t->class_id, credential_token.class_id) != 0)
        return 5;
    ac.credential_token_back++;
    return 0;
}

/* The plugin allows every algorithm and requires none. */
static const ZeroDdsPluginParticipantConfig participant_config = {
    false, false, false, false, false, false, false, 0, NULL, 0,
    {{0xffffffffu, 0}, {0xffffffffu, 0}, {0xffffffffu, 0},
     0xffffffffu, 0, 0, 0}};

static ZeroDdsPluginStatus ac_get_participant_config(void *user, uint64_t permissions,
        ZeroDdsPluginParticipantConfig *out, ZeroDdsPluginError *err) {
    (void)user; (void)permissions; (void)err;
    ac.participant_config_out++;
    *out = participant_config;
    return 0;
}

static ZeroDdsPluginStatus ac_get_topic_config(void *user, uint64_t permissions,
        const char *topic, ZeroDdsPluginTopicConfig *out, ZeroDdsPluginError *err) {
    (void)user; (void)permissions; (void)topic; (void)err;
    memset(out, 0, sizeof(*out));
    return 0;
}

static ZeroDdsPluginStatus ac_get_writer_config(void *user, uint64_t permissions,
        const ZeroDdsPluginEndpointRequest *r, ZeroDdsPluginEndpointConfig *out,
        ZeroDdsPluginError *err) {
    (void)user; (void)permissions; (void)err;
    ac.writer_config_out++;
    memset(out, 0, sizeof(*out));
    /* The payload of the topic "Protected" is protected, and encrypted. */
    if (strcmp(r->topic_name, "Protected") == 0) {
        out->is_payload_protected = true;
        out->plugin_endpoint_attributes = 2u; /* IS_PAYLOAD_ENCRYPTED */
    }
    return 0;
}

static ZeroDdsPluginStatus ac_get_reader_config(void *user, uint64_t permissions,
        const ZeroDdsPluginEndpointRequest *r, ZeroDdsPluginEndpointConfig *out,
        ZeroDdsPluginError *err) {
    (void)user; (void)permissions; (void)r; (void)err;
    ac.reader_config_out++;
    memset(out, 0, sizeof(*out));
    return 0;
}

static ZeroDdsPluginStatus ac_return_participant_config(void *user,
        const ZeroDdsPluginParticipantConfig *c, ZeroDdsPluginError *err) {
    (void)user; (void)c; (void)err;
    ac.participant_config_back++;
    return 0;
}

static ZeroDdsPluginStatus ac_return_topic_config(void *user,
        const ZeroDdsPluginTopicConfig *c, ZeroDdsPluginError *err) {
    (void)user; (void)c; (void)err;
    ac.topic_config_back++;
    return 0;
}

static ZeroDdsPluginStatus ac_return_writer_config(void *user,
        const ZeroDdsPluginEndpointConfig *c, ZeroDdsPluginError *err) {
    (void)user; (void)c; (void)err;
    ac.writer_config_back++;
    return 0;
}

static ZeroDdsPluginStatus ac_return_reader_config(void *user,
        const ZeroDdsPluginEndpointConfig *c, ZeroDdsPluginError *err) {
    (void)user; (void)c; (void)err;
    ac.reader_config_back++;
    return 0;
}

static void ac_destroy(void *user) {
    (void)user;
    ac.destroyed++;
}

/* ---------------------------------------------------------- Authentication */

static const uint8_t identity_token[] = "C-IDENTITY-TOKEN";
static const uint8_t status_token[] = "C-IDENTITY-STATUS-TOKEN";
static const uint8_t permissions_announced[] = "C-PERMISSIONS-TOKEN";

static const char *au_class_id(void *user) {
    (void)user;
    return "DDS:Auth:C";
}

static ZeroDdsPluginStatus au_validate_local(void *user, uint32_t domain,
        const ZeroDdsPluginProperty *props, size_t n,
        const ZeroDdsPluginBinaryProperty *bins, size_t nb,
        const uint8_t guid[16], int32_t *out_result, uint64_t *out_identity,
        uint8_t out_guid[16], ZeroDdsPluginError *err) {
    size_t i;
    (void)user; (void)domain; (void)bins; (void)nb; (void)err;
    au.validate_local++;
    for (i = 0; i < n; i++) {
        if (strcmp(props[i].name, "dds.sec.access.governance") == 0) au.has_governance_property = 1;
    }
    *out_result = ZERODDS_VALIDATION_OK;
    *out_identity = 100;
    memcpy(out_guid, guid, 16);
    return 0;
}

static ZeroDdsPluginStatus au_identity_token(void *user, uint64_t identity,
        ZeroDdsPluginBytes *out, ZeroDdsPluginError *err) {
    (void)user; (void)identity; (void)err;
    au.identity_token_out++;
    out->data = identity_token;
    out->len = sizeof(identity_token) - 1;
    return 0;
}

static ZeroDdsPluginStatus au_status_token(void *user, uint64_t identity,
        ZeroDdsPluginBytes *out, ZeroDdsPluginError *err) {
    (void)user; (void)identity; (void)err;
    au.status_token_out++;
    out->data = status_token;
    out->len = sizeof(status_token) - 1;
    return 0;
}

static ZeroDdsPluginStatus au_set_config(void *user, uint64_t identity,
        const ZeroDdsPluginAlgorithmMasks *config, ZeroDdsPluginAlgorithmMasks *out,
        ZeroDdsPluginError *err) {
    (void)user; (void)identity; (void)err;
    au.config_set++;
    if (au.credential_set > 0) au.credential_before_config = 1;
    *out = *config;
    return 0;
}

static ZeroDdsPluginStatus au_set_credential(void *user, uint64_t identity,
        const ZeroDdsPluginBytes *credential, const ZeroDdsPluginBytes *token,
        ZeroDdsPluginError *err) {
    (void)user; (void)identity; (void)credential; (void)token; (void)err;
    au.credential_set++;
    return 0;
}

static ZeroDdsPluginStatus au_validate_remote(void *user, uint64_t local,
        const uint8_t guid[16], const ZeroDdsPluginBytes *token,
        const ZeroDdsPluginBytes *request, int32_t *out_result, uint64_t *out_identity,
        ZeroDdsPluginBytes *out_request, ZeroDdsPluginError *err) {
    (void)user; (void)local; (void)guid; (void)token; (void)request; (void)err;
    *out_result = ZERODDS_VALIDATION_FAILED;
    *out_identity = 0;
    out_request->data = NULL;
    out_request->len = 0;
    return 0;
}

static ZeroDdsPluginStatus au_begin_request(void *user, uint64_t a, uint64_t b,
        const ZeroDdsPluginBytes *data, uint64_t *out_handshake,
        ZeroDdsPluginHandshakeStep *step, ZeroDdsPluginError *err) {
    (void)user; (void)a; (void)b; (void)data; (void)out_handshake; (void)step; (void)err;
    return 6; /* not implemented: the test has no peer */
}

static ZeroDdsPluginStatus au_begin_reply(void *user, uint64_t a, uint64_t b,
        const ZeroDdsPluginBytes *request, const ZeroDdsPluginBytes *data,
        uint64_t *out_handshake, ZeroDdsPluginHandshakeStep *step,
        ZeroDdsPluginError *err) {
    (void)user; (void)a; (void)b; (void)request; (void)data; (void)out_handshake;
    (void)step; (void)err;
    return 6;
}

static ZeroDdsPluginStatus au_process(void *user, uint64_t handshake,
        const ZeroDdsPluginBytes *token, ZeroDdsPluginHandshakeStep *step,
        ZeroDdsPluginError *err) {
    (void)user; (void)handshake; (void)token; (void)step; (void)err;
    return 6;
}

static ZeroDdsPluginStatus au_shared_secret(void *user, uint64_t handshake,
        uint64_t *out, ZeroDdsPluginError *err) {
    (void)user; (void)handshake; (void)out; (void)err;
    return 6;
}

static ZeroDdsPluginStatus au_peer_credential(void *user, uint64_t handshake,
        ZeroDdsPluginBytes *out, ZeroDdsPluginError *err) {
    (void)user; (void)handshake; (void)out; (void)err;
    return 6;
}

static ZeroDdsPluginStatus au_set_listener(void *user,
        const ZeroDdsPluginAuthenticationListener *l, ZeroDdsPluginError *err) {
    (void)user; (void)l; (void)err;
    au.listener_set++;
    return 0;
}

static ZeroDdsPluginStatus au_return_identity_token(void *user,
        const ZeroDdsPluginBytes *t, ZeroDdsPluginError *err) {
    (void)user; (void)err;
    if (t->data != identity_token) return 5;
    au.identity_token_back++;
    return 0;
}

static ZeroDdsPluginStatus au_return_status_token(void *user,
        const ZeroDdsPluginBytes *t, ZeroDdsPluginError *err) {
    (void)user; (void)err;
    if (t->data != status_token) return 5;
    au.status_token_back++;
    return 0;
}

static ZeroDdsPluginStatus au_return_peer_credential(void *user,
        const ZeroDdsPluginBytes *t, ZeroDdsPluginError *err) {
    (void)user; (void)t; (void)err;
    return 0;
}

static ZeroDdsPluginStatus au_return_handle(void *user, uint64_t handle,
        ZeroDdsPluginError *err) {
    (void)user; (void)handle; (void)err;
    return 0;
}

static void au_permissions_token(void *user, ZeroDdsPluginBytes *out) {
    (void)user;
    out->data = permissions_announced;
    out->len = sizeof(permissions_announced) - 1;
}

static void au_destroy(void *user) {
    (void)user;
    au.destroyed++;
}

/* ----------------------------------------------------------- Cryptographic */

static ZeroDdsPluginStatus cr_register_participant(void *user, uint64_t identity,
        uint64_t permissions, const ZeroDdsPluginProperty *props, size_t n,
        const ZeroDdsPluginParticipantConfig *config, uint64_t *out,
        ZeroDdsPluginParticipantAlgorithmInfo *info, ZeroDdsPluginError *err) {
    (void)user; (void)identity; (void)permissions; (void)props; (void)n; (void)err;
    cr.register_participant++;
    cr.participant_axk = config->is_rtps_axk_protected ? 1 : 0;
    *out = 500;
    memset(info, 0, sizeof(*info));
    return 0;
}

static ZeroDdsPluginStatus cr_register_remote_participant(void *user, uint64_t local,
        uint64_t identity, uint64_t permissions, uint64_t secret, uint64_t *out,
        ZeroDdsPluginError *err) {
    (void)user; (void)local; (void)identity; (void)permissions; (void)secret; (void)err;
    *out = 501;
    return 0;
}

static void cr_note_properties(const ZeroDdsPluginProperty *props, size_t n, int writer) {
    size_t i;
    for (i = 0; i < n; i++) {
        if (strcmp(props[i].name, "dds.sec.builtin_endpoint_name") == 0) {
            if (writer && strcmp(props[i].value, "BuiltinParticipantVolatileMessageSecureWriter") == 0)
                cr.volatile_writer_named = 1;
            if (!writer && strcmp(props[i].value, "BuiltinParticipantVolatileMessageSecureReader") == 0)
                cr.volatile_reader_named = 1;
        }
    }
}

static ZeroDdsPluginStatus cr_register_datawriter(void *user, uint64_t participant,
        const ZeroDdsPluginProperty *props, size_t n,
        const ZeroDdsPluginEndpointConfig *config, const uint8_t guid[16],
        uint64_t *out, ZeroDdsPluginEndpointAlgorithmInfo *info, ZeroDdsPluginError *err) {
    (void)user; (void)participant; (void)guid; (void)err;
    cr.register_datawriter++;
    cr_note_properties(props, n, 1);
    if (config->is_payload_protected) cr.protected_writer_registered = 1;
    *out = 600 + (uint64_t)cr.register_datawriter;
    memset(info, 0, sizeof(*info));
    return 0;
}

static ZeroDdsPluginStatus cr_register_datareader(void *user, uint64_t participant,
        const ZeroDdsPluginProperty *props, size_t n,
        const ZeroDdsPluginEndpointConfig *config, const uint8_t guid[16],
        uint64_t *out, ZeroDdsPluginEndpointAlgorithmInfo *info, ZeroDdsPluginError *err) {
    (void)user; (void)participant; (void)config; (void)guid; (void)err;
    cr.register_datareader++;
    cr_note_properties(props, n, 0);
    *out = 700 + (uint64_t)cr.register_datareader;
    memset(info, 0, sizeof(*info));
    return 0;
}

static ZeroDdsPluginStatus cr_register_remote_datareader(void *user, uint64_t local,
        uint64_t participant, uint64_t secret, bool relay_only, uint64_t *out,
        ZeroDdsPluginError *err) {
    (void)user; (void)local; (void)participant; (void)secret; (void)relay_only; (void)err;
    *out = 800;
    return 0;
}

static ZeroDdsPluginStatus cr_register_remote_datawriter(void *user, uint64_t local,
        uint64_t participant, uint64_t secret, uint64_t *out, ZeroDdsPluginError *err) {
    (void)user; (void)local; (void)participant; (void)secret; (void)err;
    *out = 801;
    return 0;
}

static ZeroDdsPluginStatus cr_revise(void *user, uint64_t participant, int32_t *out,
        ZeroDdsPluginError *err) {
    (void)user; (void)participant; (void)err;
    *out = 1;
    return 0;
}

static ZeroDdsPluginStatus cr_activate(void *user, uint64_t participant, int32_t revision,
        bool *out, ZeroDdsPluginError *err) {
    (void)user; (void)participant; (void)revision; (void)err;
    *out = true;
    return 0;
}

static ZeroDdsPluginStatus cr_unregister(void *user, uint64_t handle, bool *out,
        ZeroDdsPluginError *err) {
    (void)user; (void)handle; (void)err;
    *out = true;
    return 0;
}

static ZeroDdsPluginStatus cr_create_tokens(void *user, uint64_t a, uint64_t b,
        int32_t revision, ZeroDdsPluginCryptoTokens *out, ZeroDdsPluginError *err) {
    (void)user; (void)a; (void)b; (void)revision; (void)err;
    out->present = false;
    out->tokens = NULL;
    out->n = 0;
    return 0;
}

static ZeroDdsPluginStatus cr_set_tokens(void *user, uint64_t a, uint64_t b,
        const ZeroDdsPluginDataHolder *tokens, size_t n, bool *out, ZeroDdsPluginError *err) {
    (void)user; (void)a; (void)b; (void)tokens; (void)n; (void)err;
    *out = true;
    return 0;
}

static ZeroDdsPluginStatus cr_return_tokens(void *user, const ZeroDdsPluginDataHolder *tokens,
        size_t n, bool *out, ZeroDdsPluginError *err) {
    (void)user; (void)tokens; (void)n; (void)err;
    *out = true;
    return 0;
}

/* The transform of this plugin puts one octet 0xC0 before what it protects. */
static uint8_t cr_buffer[65536];

static void cr_wrap(const ZeroDdsPluginBytes *plain, ZeroDdsPluginBytes *out) {
    size_t n = plain->len < sizeof(cr_buffer) - 1 ? plain->len : sizeof(cr_buffer) - 1;
    cr_buffer[0] = 0xC0;
    memcpy(cr_buffer + 1, plain->data, n);
    out->data = cr_buffer;
    out->len = n + 1;
}

static void cr_unwrap(const ZeroDdsPluginBytes *encoded, ZeroDdsPluginBytes *out) {
    out->data = encoded->len > 0 ? encoded->data + 1 : encoded->data;
    out->len = encoded->len > 0 ? encoded->len - 1 : 0;
}

static ZeroDdsPluginStatus cr_encode_payload(void *user, const ZeroDdsPluginBytes *plain,
        uint64_t writer, ZeroDdsPluginEncodedPayload *out, ZeroDdsPluginError *err) {
    (void)user; (void)writer; (void)err;
    cr.encode_payload++;
    out->present = true;
    cr_wrap(plain, &out->encoded);
    out->extra_inline_qos.data = NULL;
    out->extra_inline_qos.len = 0;
    return 0;
}

static ZeroDdsPluginStatus cr_encode_datawriter(void *user, const ZeroDdsPluginBytes *plain,
        uint64_t writer, const uint64_t *readers, size_t n, size_t *index,
        ZeroDdsPluginBytes *out, bool *ok, ZeroDdsPluginError *err) {
    (void)user; (void)writer; (void)readers; (void)n; (void)index; (void)err;
    cr_wrap(plain, out);
    *ok = true;
    return 0;
}

static ZeroDdsPluginStatus cr_encode_datareader(void *user, const ZeroDdsPluginBytes *plain,
        uint64_t reader, const uint64_t *writers, size_t n, bool *present,
        ZeroDdsPluginBytes *out, ZeroDdsPluginError *err) {
    (void)user; (void)reader; (void)writers; (void)n; (void)err;
    cr_wrap(plain, out);
    *present = true;
    return 0;
}

static ZeroDdsPluginStatus cr_encode_message(void *user, const ZeroDdsPluginBytes *plain,
        uint64_t participant, const uint64_t *receivers, size_t n, size_t *index,
        bool psk, ZeroDdsPluginBytes *out, bool *ok, ZeroDdsPluginError *err) {
    (void)user; (void)participant; (void)receivers; (void)n; (void)index; (void)psk; (void)err;
    cr_wrap(plain, out);
    *ok = true;
    return 0;
}

static ZeroDdsPluginStatus cr_decode_message(void *user, const ZeroDdsPluginBytes *encoded,
        uint64_t receiving, uint64_t sending, bool *present, ZeroDdsPluginBytes *out,
        ZeroDdsPluginError *err) {
    (void)user; (void)receiving; (void)sending; (void)err;
    cr_unwrap(encoded, out);
    *present = true;
    return 0;
}

static ZeroDdsPluginStatus cr_preprocess(void *user, const ZeroDdsPluginBytes *encoded,
        uint64_t receiving, uint64_t sending, bool *present, uint64_t *writer,
        uint64_t *reader, int32_t *category, ZeroDdsPluginError *err) {
    (void)user; (void)encoded; (void)receiving; (void)sending; (void)err;
    *present = true;
    *writer = 0;
    *reader = 0;
    *category = ZERODDS_SUBMESSAGE_DATAWRITER;
    return 0;
}

static ZeroDdsPluginStatus cr_decode_submessage(void *user, const ZeroDdsPluginBytes *encoded,
        uint64_t a, uint64_t b, bool *present, ZeroDdsPluginBytes *out, ZeroDdsPluginError *err) {
    (void)user; (void)a; (void)b; (void)err;
    cr_unwrap(encoded, out);
    *present = true;
    return 0;
}

static ZeroDdsPluginStatus cr_decode_payload(void *user, const ZeroDdsPluginBytes *encoded,
        const ZeroDdsPluginBytes *inline_qos, uint64_t reader, uint64_t writer,
        bool *present, ZeroDdsPluginBytes *out, ZeroDdsPluginError *err) {
    (void)user; (void)inline_qos; (void)reader; (void)writer; (void)err;
    cr_unwrap(encoded, out);
    *present = true;
    return 0;
}

static void cr_destroy(void *user) {
    (void)user;
    cr.destroyed++;
}

/* ---------------------------------------------------------------- Logging */

static const char *lg_class_id(void *user) {
    (void)user;
    return "DDS:Logging:C";
}

static ZeroDdsPluginStatus lg_set_log_options(void *user,
        const ZeroDdsPluginLogOptions *o, ZeroDdsPluginError *err) {
    (void)user; (void)o; (void)err;
    lg.options++;
    return 0;
}

static ZeroDdsPluginStatus lg_enable_logging(void *user, ZeroDdsPluginError *err) {
    (void)user; (void)err;
    lg.enabled++;
    return 0;
}

static ZeroDdsPluginStatus lg_set_listener(void *user,
        const ZeroDdsPluginLoggerListener *l, ZeroDdsPluginError *err) {
    (void)user; (void)l; (void)err;
    return 0;
}

static void lg_log(void *user, int32_t level, const uint8_t participant[16],
        const char *category, const char *message) {
    (void)user; (void)level; (void)participant; (void)message;
    lg.messages++;
    if (strcmp(category, "access.create_datawriter.denied") == 0) lg.denied++;
}

static void lg_destroy(void *user) {
    (void)user;
    lg.destroyed++;
}

/* ------------------------------------------------------------ DataTagging */

static const ZeroDdsPluginDataTag tags[] = {{"owner", "c-plugin"}};

static const char *tg_class_id(void *user) {
    (void)user;
    return "DDS:Tagging:C";
}

static void tg_set_tags(void *user, const uint8_t guid[16],
        const ZeroDdsPluginDataTag *t, size_t n) {
    (void)user; (void)t; (void)n;
    tg.set_tags++;
    memcpy(tg.guid, guid, 16);
}

static void tg_get_tags(void *user, const uint8_t guid[16],
        const ZeroDdsPluginDataTag **out, size_t *n) {
    (void)user; (void)guid;
    tg.get_tags++;
    *out = tags;
    *n = 1;
}

static void tg_destroy(void *user) {
    (void)user;
    tg.destroyed++;
}

/* ------------------------------------------------------------------- main */

#define CHECK(cond) \
    do { \
        if (!(cond)) { \
            fprintf(stderr, "check failed: %s (line %d)\n", #cond, __LINE__); \
            return 1; \
        } \
    } while (0)

int main(int argc, char **argv) {
    if (argc != 8) {
        fprintf(stderr, "usage: %s domain ca cert key permissions_ca governance permissions\n",
                argv[0]);
        return 2;
    }
    ZeroDdsPluginAccessControl access = {
        ZERODDS_PLUGIN_ABI_VERSION, NULL,
        ac_class_id,
        ac_validate_local, ac_validate_remote,
        ac_check_create_participant, ac_check_create_datawriter,
        ac_check_create_datareader, ac_check_create_topic,
        ac_check_local_register, ac_check_local_dispose,
        ac_check_remote_participant, ac_check_remote_datawriter,
        ac_check_remote_datareader, ac_check_remote_topic,
        ac_check_local_datawriter_match, ac_check_local_datareader_match,
        ac_check_remote_register, ac_check_remote_dispose,
        ac_check_relay,
        ac_get_permissions_token, ac_get_credential_token, ac_set_listener,
        ac_return_permissions_token, ac_return_credential_token,
        ac_get_participant_config, ac_get_topic_config,
        ac_get_writer_config, ac_get_reader_config,
        ac_return_participant_config, ac_return_topic_config,
        ac_return_writer_config, ac_return_reader_config,
        ac_destroy};
    ZeroDdsPluginLogging logging = {
        ZERODDS_PLUGIN_ABI_VERSION, NULL, lg_class_id, lg_set_log_options,
        lg_enable_logging, lg_set_listener, lg_log, lg_destroy};
    ZeroDdsPluginDataTagging tagging = {
        ZERODDS_PLUGIN_ABI_VERSION, NULL, tg_class_id, tg_set_tags, tg_get_tags,
        tg_destroy};

    ZeroDdsPluginAuthentication authentication;
    ZeroDdsPluginCryptographic cryptographic;
    memset(&authentication, 0, sizeof(authentication));
    memset(&cryptographic, 0, sizeof(cryptographic));
    authentication.abi_version = ZERODDS_PLUGIN_ABI_VERSION;
    authentication.plugin_class_id = au_class_id;
    authentication.validate_local_identity = au_validate_local;
    authentication.get_identity_token = au_identity_token;
    authentication.get_identity_status_token = au_status_token;
    authentication.set_participant_security_config = au_set_config;
    authentication.set_permissions_credential_and_token = au_set_credential;
    authentication.validate_remote_identity = au_validate_remote;
    authentication.begin_handshake_request = au_begin_request;
    authentication.begin_handshake_reply = au_begin_reply;
    authentication.process_handshake = au_process;
    authentication.get_shared_secret = au_shared_secret;
    authentication.get_authenticated_peer_credential_token = au_peer_credential;
    authentication.set_listener = au_set_listener;
    authentication.return_identity_token = au_return_identity_token;
    authentication.return_identity_status_token = au_return_status_token;
    authentication.return_authenticated_peer_credential_token = au_return_peer_credential;
    authentication.return_handshake_handle = au_return_handle;
    authentication.return_identity_handle = au_return_handle;
    authentication.return_sharedsecret_handle = au_return_handle;
    authentication.get_permissions_token = au_permissions_token;
    authentication.destroy = au_destroy;
    cryptographic.abi_version = ZERODDS_PLUGIN_ABI_VERSION;
    cryptographic.register_local_participant = cr_register_participant;
    cryptographic.register_matched_remote_participant = cr_register_remote_participant;
    cryptographic.register_local_datawriter = cr_register_datawriter;
    cryptographic.register_matched_remote_datareader = cr_register_remote_datareader;
    cryptographic.register_local_datareader = cr_register_datareader;
    cryptographic.register_matched_remote_datawriter = cr_register_remote_datawriter;
    cryptographic.revise_local_entity_keys = cr_revise;
    cryptographic.activate_key_revision = cr_activate;
    cryptographic.unregister_participant = cr_unregister;
    cryptographic.unregister_datawriter = cr_unregister;
    cryptographic.unregister_datareader = cr_unregister;
    cryptographic.create_local_participant_crypto_tokens = cr_create_tokens;
    cryptographic.set_remote_participant_crypto_tokens = cr_set_tokens;
    cryptographic.create_local_datawriter_crypto_tokens = cr_create_tokens;
    cryptographic.set_remote_datawriter_crypto_tokens = cr_set_tokens;
    cryptographic.create_local_datareader_crypto_tokens = cr_create_tokens;
    cryptographic.set_remote_datareader_crypto_tokens = cr_set_tokens;
    cryptographic.return_crypto_tokens = cr_return_tokens;
    cryptographic.encode_serialized_payload = cr_encode_payload;
    cryptographic.encode_datawriter_submessage = cr_encode_datawriter;
    cryptographic.encode_datareader_submessage = cr_encode_datareader;
    cryptographic.encode_rtps_message = cr_encode_message;
    cryptographic.decode_rtps_message = cr_decode_message;
    cryptographic.preprocess_secure_submsg = cr_preprocess;
    cryptographic.decode_datawriter_submessage = cr_decode_submessage;
    cryptographic.decode_datareader_submessage = cr_decode_submessage;
    cryptographic.decode_serialized_payload = cr_decode_payload;
    cryptographic.destroy = cr_destroy;

    zerodds_ZeroDdsSecurityConfig *cfg = zerodds_security_config_create();
    CHECK(cfg != NULL);
    CHECK(zerodds_security_set_identity_ca_path(cfg, argv[2]) == 0);
    CHECK(zerodds_security_set_identity_cert_path(cfg, argv[3]) == 0);
    CHECK(zerodds_security_set_private_key_path(cfg, argv[4]) == 0);
    CHECK(zerodds_security_set_permissions_ca_path(cfg, argv[5]) == 0);
    CHECK(zerodds_security_set_governance_path(cfg, argv[6]) == 0);
    CHECK(zerodds_security_set_permissions_path(cfg, argv[7]) == 0);

    /* A plugin whose ABI revision is not this one is refused. */
    ZeroDdsPluginAccessControl stale = access;
    stale.abi_version = ZERODDS_PLUGIN_ABI_VERSION + 1;
    CHECK(zerodds_security_set_access_control_plugin(cfg, &stale) != 0);
    /* A plugin with a function missing is refused. */
    ZeroDdsPluginAccessControl incomplete = access;
    incomplete.check_create_datawriter = NULL;
    CHECK(zerodds_security_set_access_control_plugin(cfg, &incomplete) != 0);

    CHECK(zerodds_security_set_access_control_plugin(cfg, &access) == 0);
    CHECK(zerodds_security_set_logging_plugin(cfg, &logging) == 0);
    CHECK(zerodds_security_set_data_tagging_plugin(cfg, &tagging) == 0);
    /* A Cryptographic plugin without an Authentication plugin is refused when
     * the runtime is made; a plugin with a function missing is refused now. */
    ZeroDdsPluginAuthentication partial_auth = authentication;
    partial_auth.process_handshake = NULL;
    CHECK(zerodds_security_set_authentication_plugin(cfg, &partial_auth) != 0);
    ZeroDdsPluginCryptographic stale_crypto = cryptographic;
    stale_crypto.abi_version = ZERODDS_PLUGIN_ABI_VERSION + 1;
    CHECK(zerodds_security_set_cryptographic_plugin(cfg, &stale_crypto) != 0);
    CHECK(zerodds_security_set_authentication_plugin(cfg, &authentication) == 0);
    CHECK(zerodds_security_set_cryptographic_plugin(cfg, &cryptographic) == 0);

    zerodds_ZeroDdsRuntime *rt =
        zerodds_runtime_create_secure((uint32_t)atoi(argv[1]), cfg);
    CHECK(rt != NULL);

    /* 9.3.2.11: the Authentication plugin validates the identity, with the
     * paths of the configuration as the properties of Table 50; its tokens are
     * given back after the copy. */
    CHECK(au.validate_local == 1);
    CHECK(au.has_governance_property == 1);
    CHECK(au.identity_token_out >= 1);
    CHECK(au.identity_token_back == au.identity_token_out);
    CHECK(au.status_token_out >= 1);
    CHECK(au.status_token_back == au.status_token_out);
    /* 9.8.1 steps 8 and 10: the credential goes to the plugin before the
     * ParticipantSecurityConfig. */
    CHECK(au.credential_set >= 1);
    CHECK(au.config_set >= 1);
    CHECK(au.credential_before_config == 1);
    /* 9.8.1 step 2: the plugin validates the local permissions. */
    CHECK(ac.validate_local == 1);
    /* 9.8.1 step 3: check_create_participant with that handle. */
    CHECK(ac.create_participant == 1);
    /* 9.8.1 steps 6 and 7: the tokens, given back after the copy. */
    CHECK(ac.permissions_token_out >= 1);
    CHECK(ac.permissions_token_back == ac.permissions_token_out);
    CHECK(ac.credential_token_out >= 1);
    CHECK(ac.credential_token_back == ac.credential_token_out);
    /* 9.8.1 step 9: the participant config; the runtime keeps it as long as
     * the participant lives. */
    CHECK(ac.participant_config_out >= 1);
    CHECK(ac.listener_set >= 1);

    /* 9.8.6 steps 5 and 6: the DataWriter. */
    zerodds_ZeroDdsWriter *allowed = zerodds_writer_create(rt, "Allowed", "RawBytes", 0);
    CHECK(allowed != NULL);
    CHECK(ac.create_writer >= 1);
    CHECK(ac.writer_config_out >= 1);
    CHECK(ac.writer_config_back == ac.writer_config_out);
    zerodds_ZeroDdsWriter *refused = zerodds_writer_create(rt, FORBIDDEN, "RawBytes", 0);
    CHECK(refused == NULL);
    CHECK(ac.refused_writer >= 1);

    /* 9.8.6 steps 9 and 10: the DataReader. */
    /* 9.8.6 step 6: a topic whose EndpointSecurityConfig protects the payload
     * gets a key from the Cryptographic plugin. Nothing is encoded here: a
     * payload is encoded for a matched reader, and this runtime has none. */
    {
        const uint8_t sample[] = {1, 2, 3, 4};
        zerodds_ZeroDdsWriter *protected_writer =
            zerodds_writer_create(rt, "Protected", "RawBytes", 0);
        CHECK(protected_writer != NULL);
        CHECK(cr.protected_writer_registered == 1);
        CHECK(zerodds_writer_write(protected_writer, sample, sizeof(sample)) == 0);
        zerodds_writer_destroy(protected_writer);
    }
    /* The first key material is made when it is first needed. 9.8.1 step 11, 9.8.9.1: the Cryptographic plugin registers the
     * participant, with the config of the AccessControl plugin, and the
     * volatile message endpoints under the names of the specification. */
    CHECK(cr.register_participant == 1);
    CHECK(cr.participant_axk == 0);
    CHECK(cr.volatile_writer_named == 1);
    CHECK(cr.volatile_reader_named == 1);

    zerodds_ZeroDdsReader *reader = zerodds_reader_create(rt, "Allowed", "RawBytes", 0);
    CHECK(reader != NULL);
    CHECK(ac.create_reader >= 1);
    CHECK(ac.reader_config_out >= 1);
    CHECK(ac.reader_config_back == ac.reader_config_out);
    CHECK(zerodds_reader_create(rt, FORBIDDEN, "RawBytes", 0) == NULL);
    CHECK(ac.refused_reader >= 1);

    /* 9.7: the DataTagging plugin got the tags of the writer and gave the
     * tags it announces. */
    CHECK(tg.set_tags >= 1);
    CHECK(tg.get_tags >= 1);

    /* 9.6: the refusal was logged, through the Logging plugin. */
    CHECK(lg.messages >= 1);
    CHECK(lg.denied >= 1);

    zerodds_writer_destroy(allowed);
    zerodds_reader_destroy(reader);
    zerodds_runtime_destroy(rt);
    zerodds_security_config_destroy(cfg);

    /* The runtime released each plugin: destroy ran once. The release may
     * follow the return of the calls on a thread of the runtime. */
    for (int i = 0; i < 100 && (ac.destroyed == 0 || lg.destroyed == 0 || tg.destroyed == 0 ||
                      au.destroyed == 0 || cr.destroyed == 0);
         i++) {
        usleep(50 * 1000);
    }
    CHECK(ac.destroyed == 1);
    /* ... and gave the participant config back before. */
    CHECK(ac.participant_config_back == ac.participant_config_out);
    CHECK(lg.destroyed == 1);
    CHECK(tg.destroyed == 1);
    CHECK(au.destroyed == 1);
    CHECK(cr.destroyed == 1);
    printf("OK\n");
    return 0;
}
