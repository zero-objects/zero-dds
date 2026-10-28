/* SPDX-License-Identifier: Apache-2.0 */
/* Copyright 2026 ZeroDDS Contributors */

/*
 * zerodds_security_plugins.h -- plugins of an application in C.
 *
 * DDS-Security 1.2 clause 8 and clause 9 define the plugin interfaces
 * (AccessControl, Logging, DataTagging) and clause 11 their language
 * representations. An application that has its own plugin fills in one of
 * the structs below and hands it to the security configuration; the runtime
 * calls the plugin where the specification calls it (9.8).
 *
 *     ZeroDdsPluginAccessControl ac = { ZERODDS_PLUGIN_ABI_VERSION, ... };
 *     zerodds_security_set_access_control_plugin(cfg, &ac);
 *     zerodds_runtime_create_secure(domain, cfg);
 *
 * The runtime copies the struct. `user` is handed to every function and to
 * `destroy`, which the runtime calls once when the plugin is released (the
 * security configuration and every runtime made from it have let go of it).
 *
 * Calls
 * -----
 * Every operation of a plugin is called with the lock of its plugin held:
 * the functions of one plugin are never called concurrently. A function must
 * not call back into the runtime that calls it.
 *
 * Status
 * ------
 * An operation returns 0 on success. Any other value is a failure; it is the
 * `code` of the SecurityException (Table 28: 1 authentication failed, 2
 * access denied, 3 crypto failed, 4 invalid configuration, 5 bad argument,
 * 6 not implemented, 7 internal), and `*err` is filled in: `message` is a
 * NUL-terminated string that stays valid until the next call into the
 * plugin (may be NULL), `minor_code` the plugin's own number (0 for none).
 * A refusal is not a failure: operations that decide (check_*) return 0 and
 * set `*out_permit` to false.
 *
 * Memory
 * ------
 * Pointers the runtime passes in are valid for the call. What a plugin hands
 * out (tokens, configs) belongs to the plugin and stays valid until the
 * runtime gives it back with the matching return_* operation; the runtime
 * copies it before.
 *
 * Handles are 64-bit numbers; 0 is HandleNIL. GUIDs are 16 octets.
 */

#ifndef ZERODDS_SECURITY_PLUGINS_H
#define ZERODDS_SECURITY_PLUGINS_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* The revision of the structs in this file; `abi_version` of each plugin
 * struct must be this value, or the runtime refuses the plugin. */
#define ZERODDS_PLUGIN_ABI_VERSION 1u

typedef int32_t ZeroDdsPluginStatus;

typedef struct ZeroDdsPluginError {
    const char *message;
    int32_t minor_code;
} ZeroDdsPluginError;

/* Property_t of DDS-Security 1.2 Table 5: a name, a value, and whether it is
 * propagated. */
typedef struct ZeroDdsPluginProperty {
    const char *name;
    const char *value;
    bool propagate;
} ZeroDdsPluginProperty;

typedef struct ZeroDdsPluginBinaryProperty {
    const char *name;
    const uint8_t *value;
    size_t value_len;
    bool propagate;
} ZeroDdsPluginBinaryProperty;

/* DataHolder (Table 6): the form of every token. */
typedef struct ZeroDdsPluginDataHolder {
    const char *class_id;
    const ZeroDdsPluginProperty *properties;
    size_t n_properties;
    const ZeroDdsPluginBinaryProperty *binary_properties;
    size_t n_binary_properties;
} ZeroDdsPluginDataHolder;

/* A name and a value: a QoS entry of a request, a DataTag. */
typedef struct ZeroDdsPluginPair {
    const char *name;
    const char *value;
} ZeroDdsPluginPair;

/* (supported, required) algorithm masks (7.3.10). */
typedef struct ZeroDdsPluginMasks {
    uint32_t supported_mask;
    uint32_t required_mask;
} ZeroDdsPluginMasks;

/* The algorithms the Authentication plugin announces (7.3.11, 7.3.12). */
typedef struct ZeroDdsPluginAlgorithmMasks {
    ZeroDdsPluginMasks trust_chain;
    ZeroDdsPluginMasks message_auth;
    ZeroDdsPluginMasks key_establishment;
} ZeroDdsPluginAlgorithmMasks;

/* ParticipantSecurityAlgorithmInfo (7.3.14). */
typedef struct ZeroDdsPluginParticipantAlgorithmInfo {
    ZeroDdsPluginMasks trust_chain;
    ZeroDdsPluginMasks message_auth;
    ZeroDdsPluginMasks shared_secret;
    uint32_t cipher_supported_mask;
    uint32_t cipher_builtin_endpoints_required_mask;
    uint32_t cipher_builtin_kx_endpoints_required_mask;
    uint32_t cipher_user_endpoints_default_required_mask;
} ZeroDdsPluginParticipantAlgorithmInfo;

/* What the runtime asks the plugin about a participant (9.4.2.9.1, .3, .9). */
typedef struct ZeroDdsPluginParticipantRequest {
    uint32_t domain_id;
    const char *domain_tag;
    uint8_t participant_guid[16];
    const ZeroDdsPluginProperty *properties;
    size_t n_properties;
    bool has_identity_algorithms;
    ZeroDdsPluginAlgorithmMasks identity_algorithms;
} ZeroDdsPluginParticipantRequest;

/* ... about a topic (9.4.2.9.6, .12). */
typedef struct ZeroDdsPluginTopicRequest {
    uint32_t domain_id;
    const char *topic_name;
    const ZeroDdsPluginPair *qos;
    size_t n_qos;
} ZeroDdsPluginTopicRequest;

/* ... about a DataWriter or DataReader, local or remote (9.4.2.9.4, .5, .10,
 * .11, .13, .14): its topic, partitions, data tags and QoS. */
typedef struct ZeroDdsPluginEndpointRequest {
    uint32_t domain_id;
    const char *topic_name;
    const char *const *partitions;
    size_t n_partitions;
    const ZeroDdsPluginPair *data_tags;
    size_t n_data_tags;
    const ZeroDdsPluginPair *qos;
    size_t n_qos;
} ZeroDdsPluginEndpointRequest;

/* ParticipantSecurityConfig (9.4.2.4). */
typedef struct ZeroDdsPluginParticipantConfig {
    bool allow_unauthenticated_participants;
    bool is_access_protected;
    bool is_rtps_axk_protected;
    bool is_rtps_psk_protected;
    bool is_discovery_protected;
    bool is_liveliness_protected;
    bool is_key_revision_enabled;
    uint32_t plugin_participant_attributes;
    const ZeroDdsPluginProperty *ac_participant_properties;
    size_t n_ac_participant_properties;
    ZeroDdsPluginParticipantAlgorithmInfo algorithm_info;
} ZeroDdsPluginParticipantConfig;

/* TopicSecurityConfig (9.4.2.6). */
typedef struct ZeroDdsPluginTopicConfig {
    bool is_read_protected;
    bool is_write_protected;
    bool is_discovery_protected;
    bool is_liveliness_protected;
} ZeroDdsPluginTopicConfig;

/* EndpointSecurityConfig (9.4.2.7). */
typedef struct ZeroDdsPluginEndpointConfig {
    ZeroDdsPluginTopicConfig topic;
    bool is_submessage_protected;
    bool is_payload_protected;
    bool is_key_protected;
    uint32_t plugin_endpoint_attributes;
    const ZeroDdsPluginProperty *ac_endpoint_properties;
    size_t n_ac_endpoint_properties;
    uint32_t cipher_required_mask;
    uint32_t cipher_supported_mask;
} ZeroDdsPluginEndpointConfig;

/* Answer of check_remote_datareader (9.4.2.9.11). */
enum {
    ZERODDS_PLUGIN_DENY = 0,
    ZERODDS_PLUGIN_PERMIT = 1,
    ZERODDS_PLUGIN_RELAY_ONLY = 2
};

/* Role of check_relay. */
enum { ZERODDS_PLUGIN_RELAY_READER = 0, ZERODDS_PLUGIN_RELAY_WRITER = 1 };

/* AccessControlListener (9.4.2.10): the runtime gives it to the plugin with
 * set_listener; the plugin calls on_revoke_permissions when it revokes the
 * permissions behind `handle`. The struct stays valid until the plugin is
 * destroyed. */
typedef struct ZeroDdsPluginAccessControlListener {
    void *context;
    bool (*on_revoke_permissions)(void *context, const char *plugin, uint64_t handle);
} ZeroDdsPluginAccessControlListener;

/*
 * AccessControl plugin (9.4.2.9, Table 39). Each function is the operation
 * of the same name; a function pointer that is NULL makes the runtime refuse
 * the plugin.
 */
typedef struct ZeroDdsPluginAccessControl {
    uint32_t abi_version;
    void *user;

    const char *(*plugin_class_id)(void *user);

    ZeroDdsPluginStatus (*validate_local_permissions)(
        void *user, uint64_t identity,
        const ZeroDdsPluginParticipantRequest *participant,
        uint64_t *out_handle, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*validate_remote_permissions)(
        void *user, uint64_t local_identity, uint64_t remote_identity,
        const ZeroDdsPluginDataHolder *remote_permissions_token,
        const ZeroDdsPluginDataHolder *remote_credential_token,
        uint64_t *out_handle, ZeroDdsPluginError *err);

    ZeroDdsPluginStatus (*check_create_participant)(
        void *user, uint64_t permissions,
        const ZeroDdsPluginParticipantRequest *participant,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_create_datawriter)(
        void *user, uint64_t permissions,
        const ZeroDdsPluginEndpointRequest *request,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_create_datareader)(
        void *user, uint64_t permissions,
        const ZeroDdsPluginEndpointRequest *request,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_create_topic)(
        void *user, uint64_t permissions,
        const ZeroDdsPluginTopicRequest *request,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_local_datawriter_register_instance)(
        void *user, uint64_t permissions, const uint8_t writer[16],
        const uint8_t *key, size_t key_len,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_local_datawriter_dispose_instance)(
        void *user, uint64_t permissions, const uint8_t writer[16],
        const uint8_t *key, size_t key_len,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_remote_participant)(
        void *user, uint64_t permissions,
        const ZeroDdsPluginParticipantRequest *participant,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_remote_datawriter)(
        void *user, uint64_t permissions,
        const ZeroDdsPluginEndpointRequest *request,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_remote_datareader)(
        void *user, uint64_t permissions,
        const ZeroDdsPluginEndpointRequest *request,
        int32_t *out_decision, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_remote_topic)(
        void *user, uint64_t permissions,
        const ZeroDdsPluginTopicRequest *request,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_local_datawriter_match)(
        void *user, uint64_t writer_permissions, uint64_t reader_permissions,
        const ZeroDdsPluginEndpointRequest *publication,
        const ZeroDdsPluginEndpointRequest *subscription,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_local_datareader_match)(
        void *user, uint64_t writer_permissions, uint64_t reader_permissions,
        const ZeroDdsPluginEndpointRequest *subscription,
        const ZeroDdsPluginEndpointRequest *publication,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_remote_datawriter_register_instance)(
        void *user, uint64_t permissions, const uint8_t reader[16],
        const uint8_t publication[16], const uint8_t *key, size_t key_len,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_remote_datawriter_dispose_instance)(
        void *user, uint64_t permissions, const uint8_t reader[16],
        const uint8_t publication[16], const uint8_t *key, size_t key_len,
        bool *out_permit, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*check_relay)(
        void *user, uint64_t permissions, int32_t role,
        const ZeroDdsPluginEndpointRequest *request,
        bool *out_permit, ZeroDdsPluginError *err);

    ZeroDdsPluginStatus (*get_permissions_token)(
        void *user, uint64_t handle, ZeroDdsPluginDataHolder *out_token,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*get_permissions_credential_token)(
        void *user, uint64_t handle, ZeroDdsPluginDataHolder *out_token,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*set_listener)(
        void *user, const ZeroDdsPluginAccessControlListener *listener,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_permissions_token)(
        void *user, const ZeroDdsPluginDataHolder *token,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_permissions_credential_token)(
        void *user, const ZeroDdsPluginDataHolder *token,
        ZeroDdsPluginError *err);

    ZeroDdsPluginStatus (*get_participant_security_config)(
        void *user, uint64_t permissions,
        ZeroDdsPluginParticipantConfig *out_config, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*get_topic_security_config)(
        void *user, uint64_t permissions, const char *topic_name,
        ZeroDdsPluginTopicConfig *out_config, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*get_datawriter_security_config)(
        void *user, uint64_t permissions,
        const ZeroDdsPluginEndpointRequest *request,
        ZeroDdsPluginEndpointConfig *out_config, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*get_datareader_security_config)(
        void *user, uint64_t permissions,
        const ZeroDdsPluginEndpointRequest *request,
        ZeroDdsPluginEndpointConfig *out_config, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_participant_security_config)(
        void *user, const ZeroDdsPluginParticipantConfig *config,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_topic_security_config)(
        void *user, const ZeroDdsPluginTopicConfig *config,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_datawriter_security_config)(
        void *user, const ZeroDdsPluginEndpointConfig *config,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_datareader_security_config)(
        void *user, const ZeroDdsPluginEndpointConfig *config,
        ZeroDdsPluginError *err);

    void (*destroy)(void *user);
} ZeroDdsPluginAccessControl;

/* Levels of LogOptions (9.6.2.1.1). */
enum {
    ZERODDS_LOG_FATAL = 0,
    ZERODDS_LOG_SEVERE = 1,
    ZERODDS_LOG_ERROR = 2,
    ZERODDS_LOG_WARNING = 3,
    ZERODDS_LOG_NOTICE = 4,
    ZERODDS_LOG_INFO = 5,
    ZERODDS_LOG_DEBUG = 6,
    ZERODDS_LOG_TRACE = 7
};

/* LogOptions (Table 45). `log_file` is NULL for none. */
typedef struct ZeroDdsPluginLogOptions {
    int32_t log_level;
    const char *log_file;
    bool distribute;
} ZeroDdsPluginLogOptions;

/* LoggerListener (9.6.2.3): the plugin calls it for every message it logs. */
typedef struct ZeroDdsPluginLoggerListener {
    void *context;
    int32_t (*log_message)(void *context, int32_t level,
                           const uint8_t participant[16],
                           const char *category, const char *message);
} ZeroDdsPluginLoggerListener;

/*
 * Logging plugin (9.6.2.2, Table 46). `log` has no status: logging does not
 * fail the operation that logs.
 */
typedef struct ZeroDdsPluginLogging {
    uint32_t abi_version;
    void *user;

    const char *(*plugin_class_id)(void *user);
    ZeroDdsPluginStatus (*set_log_options)(
        void *user, const ZeroDdsPluginLogOptions *options,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*enable_logging)(void *user, ZeroDdsPluginError *err);
    /* `listener` is NULL for none; the struct stays valid until the plugin is
     * destroyed or set_listener is called again. */
    ZeroDdsPluginStatus (*set_listener)(
        void *user, const ZeroDdsPluginLoggerListener *listener,
        ZeroDdsPluginError *err);
    void (*log)(void *user, int32_t level, const uint8_t participant[16],
                const char *category, const char *message);

    void (*destroy)(void *user);
} ZeroDdsPluginLogging;

/* The tags set_tags gives and get_tags returns. */
typedef struct ZeroDdsPluginDataTag {
    const char *name;
    const char *value;
} ZeroDdsPluginDataTag;

/*
 * DataTagging plugin (9.7). `set_tags` hands the plugin the tags of an
 * endpoint; `get_tags` asks for the tags the endpoint announces: the plugin
 * returns a list that it owns until the next call of get_tags or set_tags.
 */
typedef struct ZeroDdsPluginDataTagging {
    uint32_t abi_version;
    void *user;

    const char *(*plugin_class_id)(void *user);
    void (*set_tags)(void *user, const uint8_t endpoint_guid[16],
                     const ZeroDdsPluginDataTag *tags, size_t n_tags);
    void (*get_tags)(void *user, const uint8_t endpoint_guid[16],
                     const ZeroDdsPluginDataTag **out_tags, size_t *out_n_tags);

    void (*destroy)(void *user);
} ZeroDdsPluginDataTagging;


/* ------------------------------------------------------------------------
 * Authentication and Cryptographic
 * ---------------------------------------------------------------------- */

/* A buffer of octets: a token, a message. A buffer a plugin hands out stays
 * valid until the return operation of its token, or, where there is none,
 * until the next call into the plugin; the runtime copies it before. */
typedef struct ZeroDdsPluginBytes {
    const uint8_t *data;
    size_t len;
} ZeroDdsPluginBytes;

/* ValidationResult (9.3.2.11.2 and following). */
enum {
    ZERODDS_VALIDATION_OK = 0,
    ZERODDS_VALIDATION_FAILED = 1,
    ZERODDS_VALIDATION_PENDING_RETRY = 2,
    ZERODDS_VALIDATION_PENDING_HANDSHAKE_REQUEST = 3,
    ZERODDS_VALIDATION_PENDING_HANDSHAKE_MESSAGE = 4,
    ZERODDS_VALIDATION_OK_FINAL_MESSAGE = 5
};

/* What a step of the handshake gives (begin_handshake_request,
 * begin_handshake_reply, process_handshake). */
enum {
    ZERODDS_HANDSHAKE_SEND_MESSAGE = 0,
    ZERODDS_HANDSHAKE_SEND_FINAL_MESSAGE = 1,
    ZERODDS_HANDSHAKE_COMPLETE = 2,
    ZERODDS_HANDSHAKE_WAITING_FOR_PEER = 3,
    ZERODDS_HANDSHAKE_PENDING_RETRY = 4
};

typedef struct ZeroDdsPluginHandshakeStep {
    int32_t kind;
    ZeroDdsPluginBytes token;  /* kinds SEND_MESSAGE and SEND_FINAL_MESSAGE */
    uint64_t secret;           /* kind COMPLETE: the SharedSecretHandle */
} ZeroDdsPluginHandshakeStep;

/* AuthenticationListener (9.3.2.12): the runtime gives it to the plugin with
 * set_listener (NULL for none). */
typedef struct ZeroDdsPluginAuthenticationListener {
    void *context;
    int32_t (*on_revoke_identity)(void *context, uint64_t handle);
    void (*on_status_changed)(void *context, uint64_t handle, int32_t status_kind);
} ZeroDdsPluginAuthenticationListener;

/*
 * Authentication plugin (9.3.2.11, Table 29). A token is the CDR of a
 * DataHolder (the octets that go into the participant data and the
 * handshake messages). `get_shared_secret_bytes` and
 * `get_shared_secret_challenges` may be NULL; they are needed where the
 * builtin Cryptographic plugin is used with this plugin.
 */
typedef struct ZeroDdsPluginAuthentication {
    uint32_t abi_version;
    void *user;

    const char *(*plugin_class_id)(void *user);

    ZeroDdsPluginStatus (*validate_local_identity)(
        void *user, uint32_t domain_id,
        const ZeroDdsPluginProperty *properties, size_t n_properties,
        const ZeroDdsPluginBinaryProperty *binary_properties,
        size_t n_binary_properties, const uint8_t candidate_guid[16],
        int32_t *out_result, uint64_t *out_identity,
        uint8_t out_adjusted_guid[16], ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*get_identity_token)(
        void *user, uint64_t identity, ZeroDdsPluginBytes *out_token,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*get_identity_status_token)(
        void *user, uint64_t identity, ZeroDdsPluginBytes *out_token,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*set_participant_security_config)(
        void *user, uint64_t identity, const ZeroDdsPluginAlgorithmMasks *config,
        ZeroDdsPluginAlgorithmMasks *out_adjusted, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*set_permissions_credential_and_token)(
        void *user, uint64_t identity, const ZeroDdsPluginBytes *credential_token,
        const ZeroDdsPluginBytes *permissions_token, ZeroDdsPluginError *err);
    /* `remote_auth_request_token` is NULL for none; `out_local_auth_request`
     * has a NULL `data` for none. */
    ZeroDdsPluginStatus (*validate_remote_identity)(
        void *user, uint64_t local_identity, const uint8_t remote_guid[16],
        const ZeroDdsPluginBytes *remote_identity_token,
        const ZeroDdsPluginBytes *remote_auth_request_token,
        int32_t *out_result, uint64_t *out_identity,
        ZeroDdsPluginBytes *out_local_auth_request, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*begin_handshake_request)(
        void *user, uint64_t initiator, uint64_t replier,
        const ZeroDdsPluginBytes *serialized_local_participant_data,
        uint64_t *out_handshake, ZeroDdsPluginHandshakeStep *out_step,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*begin_handshake_reply)(
        void *user, uint64_t replier, uint64_t initiator,
        const ZeroDdsPluginBytes *request_token,
        const ZeroDdsPluginBytes *serialized_local_participant_data,
        uint64_t *out_handshake, ZeroDdsPluginHandshakeStep *out_step,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*process_handshake)(
        void *user, uint64_t handshake, const ZeroDdsPluginBytes *token,
        ZeroDdsPluginHandshakeStep *out_step, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*get_shared_secret)(
        void *user, uint64_t handshake, uint64_t *out_secret,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*get_authenticated_peer_credential_token)(
        void *user, uint64_t handshake, ZeroDdsPluginBytes *out_token,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*set_listener)(
        void *user, const ZeroDdsPluginAuthenticationListener *listener,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_identity_token)(
        void *user, const ZeroDdsPluginBytes *token, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_identity_status_token)(
        void *user, const ZeroDdsPluginBytes *token, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_authenticated_peer_credential_token)(
        void *user, const ZeroDdsPluginBytes *token, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_handshake_handle)(
        void *user, uint64_t handshake, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_identity_handle)(
        void *user, uint64_t identity, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_sharedsecret_handle)(
        void *user, uint64_t secret, ZeroDdsPluginError *err);

    /* The PermissionsToken the participant announces; NULL or an empty
     * buffer for none. */
    void (*get_permissions_token)(void *user, ZeroDdsPluginBytes *out_token);
    bool (*get_shared_secret_bytes)(void *user, uint64_t secret,
                                    ZeroDdsPluginBytes *out_secret);
    bool (*get_shared_secret_challenges)(void *user, uint64_t secret,
                                         uint8_t out_challenge1[32],
                                         uint8_t out_challenge2[32]);

    void (*destroy)(void *user);
} ZeroDdsPluginAuthentication;

/* EndpointSecurityAlgorithmInfo (7.3.16). */
typedef struct ZeroDdsPluginEndpointAlgorithmInfo {
    uint32_t required_mask;
    uint32_t supported_mask;
} ZeroDdsPluginEndpointAlgorithmInfo;

/* A list of CryptoTokens a plugin owns until return_crypto_tokens. `present`
 * is false for the answer "no tokens". */
typedef struct ZeroDdsPluginCryptoTokens {
    bool present;
    const ZeroDdsPluginDataHolder *tokens;
    size_t n;
} ZeroDdsPluginCryptoTokens;

typedef struct ZeroDdsPluginEncodedPayload {
    bool present;
    ZeroDdsPluginBytes encoded;
    ZeroDdsPluginBytes extra_inline_qos;
} ZeroDdsPluginEncodedPayload;

/* SecureSubmessageCategory (9.5.1.10.5). */
enum {
    ZERODDS_SUBMESSAGE_INFO = 0,
    ZERODDS_SUBMESSAGE_DATAWRITER = 1,
    ZERODDS_SUBMESSAGE_DATAREADER = 2
};

/*
 * Cryptographic plugin (9.5.1, Tables 42 to 44): the three interfaces
 * CryptoKeyFactory, CryptoKeyExchange and CryptoTransform in one struct. The
 * ParticipantSecurityConfig of register_local_participant is passed as a
 * ZeroDdsPluginParticipantConfig (its `ac_participant_properties` are the
 * `ac_endpoint_properties` of 9.5.1.8.1), the EndpointSecurityConfig of
 * register_local_datawriter / _datareader as a ZeroDdsPluginEndpointConfig. A
 * list of handles is an array and a number; `*index` is the
 * `receiving_*_crypto_list_index` of the operation, in and out. An encode or
 * decode operation that does not apply sets `*present` / `*ok` to false.
 */
typedef struct ZeroDdsPluginCryptographic {
    uint32_t abi_version;
    void *user;

    ZeroDdsPluginStatus (*register_local_participant)(
        void *user, uint64_t identity, uint64_t permissions,
        const ZeroDdsPluginProperty *properties, size_t n_properties,
        const ZeroDdsPluginParticipantConfig *config, uint64_t *out_handle,
        ZeroDdsPluginParticipantAlgorithmInfo *out_info, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*register_matched_remote_participant)(
        void *user, uint64_t local_participant, uint64_t remote_identity,
        uint64_t remote_permissions, uint64_t shared_secret, uint64_t *out_handle,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*register_local_datawriter)(
        void *user, uint64_t participant, const ZeroDdsPluginProperty *properties,
        size_t n_properties, const ZeroDdsPluginEndpointConfig *config,
        const uint8_t endpoint_guid[16], uint64_t *out_handle,
        ZeroDdsPluginEndpointAlgorithmInfo *out_info, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*register_matched_remote_datareader)(
        void *user, uint64_t local_datawriter, uint64_t remote_participant,
        uint64_t shared_secret, bool relay_only, uint64_t *out_handle,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*register_local_datareader)(
        void *user, uint64_t participant, const ZeroDdsPluginProperty *properties,
        size_t n_properties, const ZeroDdsPluginEndpointConfig *config,
        const uint8_t endpoint_guid[16], uint64_t *out_handle,
        ZeroDdsPluginEndpointAlgorithmInfo *out_info, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*register_matched_remote_datawriter)(
        void *user, uint64_t local_datareader, uint64_t remote_participant,
        uint64_t shared_secret, uint64_t *out_handle, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*revise_local_entity_keys)(
        void *user, uint64_t participant, int32_t *out_revision,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*activate_key_revision)(
        void *user, uint64_t participant, int32_t revision, bool *out_ok,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*unregister_participant)(
        void *user, uint64_t handle, bool *out_ok, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*unregister_datawriter)(
        void *user, uint64_t handle, bool *out_ok, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*unregister_datareader)(
        void *user, uint64_t handle, bool *out_ok, ZeroDdsPluginError *err);

    ZeroDdsPluginStatus (*create_local_participant_crypto_tokens)(
        void *user, uint64_t local_participant, uint64_t remote_participant,
        int32_t key_revision, ZeroDdsPluginCryptoTokens *out,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*set_remote_participant_crypto_tokens)(
        void *user, uint64_t local_participant, uint64_t remote_participant,
        const ZeroDdsPluginDataHolder *tokens, size_t n, bool *out_ok,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*create_local_datawriter_crypto_tokens)(
        void *user, uint64_t local_datawriter, uint64_t remote_datareader,
        int32_t key_revision, ZeroDdsPluginCryptoTokens *out,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*set_remote_datawriter_crypto_tokens)(
        void *user, uint64_t local_datareader, uint64_t remote_datawriter,
        const ZeroDdsPluginDataHolder *tokens, size_t n, bool *out_ok,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*create_local_datareader_crypto_tokens)(
        void *user, uint64_t local_datareader, uint64_t remote_datawriter,
        int32_t key_revision, ZeroDdsPluginCryptoTokens *out,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*set_remote_datareader_crypto_tokens)(
        void *user, uint64_t local_datawriter, uint64_t remote_datareader,
        const ZeroDdsPluginDataHolder *tokens, size_t n, bool *out_ok,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*return_crypto_tokens)(
        void *user, const ZeroDdsPluginDataHolder *tokens, size_t n,
        bool *out_ok, ZeroDdsPluginError *err);

    ZeroDdsPluginStatus (*encode_serialized_payload)(
        void *user, const ZeroDdsPluginBytes *plain, uint64_t sending_datawriter,
        ZeroDdsPluginEncodedPayload *out, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*encode_datawriter_submessage)(
        void *user, const ZeroDdsPluginBytes *plain_submessage,
        uint64_t sending_datawriter, const uint64_t *receiving_datareaders,
        size_t n_receivers, size_t *index, ZeroDdsPluginBytes *out_encoded,
        bool *out_ok, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*encode_datareader_submessage)(
        void *user, const ZeroDdsPluginBytes *plain_submessage,
        uint64_t sending_datareader, const uint64_t *receiving_datawriters,
        size_t n_receivers, bool *out_present, ZeroDdsPluginBytes *out_encoded,
        ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*encode_rtps_message)(
        void *user, const ZeroDdsPluginBytes *plain_message,
        uint64_t sending_participant, const uint64_t *receiving_participants,
        size_t n_receivers, size_t *index, bool transform_with_psk,
        ZeroDdsPluginBytes *out_encoded, bool *out_ok, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*decode_rtps_message)(
        void *user, const ZeroDdsPluginBytes *encoded,
        uint64_t receiving_participant, uint64_t sending_participant,
        bool *out_present, ZeroDdsPluginBytes *out_plain, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*preprocess_secure_submsg)(
        void *user, const ZeroDdsPluginBytes *encoded_submessage,
        uint64_t receiving_participant, uint64_t sending_participant,
        bool *out_present, uint64_t *out_datawriter, uint64_t *out_datareader,
        int32_t *out_category, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*decode_datawriter_submessage)(
        void *user, const ZeroDdsPluginBytes *encoded_submessage,
        uint64_t receiving_datareader, uint64_t sending_datawriter,
        bool *out_present, ZeroDdsPluginBytes *out_plain, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*decode_datareader_submessage)(
        void *user, const ZeroDdsPluginBytes *encoded_submessage,
        uint64_t receiving_datawriter, uint64_t sending_datareader,
        bool *out_present, ZeroDdsPluginBytes *out_plain, ZeroDdsPluginError *err);
    ZeroDdsPluginStatus (*decode_serialized_payload)(
        void *user, const ZeroDdsPluginBytes *encoded,
        const ZeroDdsPluginBytes *inline_qos, uint64_t receiving_datareader,
        uint64_t sending_datawriter, bool *out_present,
        ZeroDdsPluginBytes *out_plain, ZeroDdsPluginError *err);

    void (*destroy)(void *user);
} ZeroDdsPluginCryptographic;

#ifdef __cplusplus
}
#endif

#endif /* ZERODDS_SECURITY_PLUGINS_H */
