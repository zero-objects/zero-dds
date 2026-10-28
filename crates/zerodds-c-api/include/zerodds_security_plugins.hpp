// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// zerodds_security_plugins.hpp -- plugins of an application in C++.
//
// DDS-Security 1.2 clause 11.4 gives the C++ form of the plugin interfaces
// of clause 9. This header is that form for the plugins a participant takes
// from the application: an abstract class for each of AccessControl,
// Logging and DataTagging, with the operations of Table 39, Table 46 and
// clause 9.7, and a function that places an object of such a class into a
// security configuration.
//
//     class MyAccess : public zerodds::security::AccessControlPlugin { ... };
//
//     zerodds::security::set_access_control_plugin(
//         cfg, std::unique_ptr<AccessControlPlugin>(new MyAccess()));
//     zerodds_runtime_create_secure(domain, cfg);
//
// The runtime owns the object from then on and deletes it when it lets go of
// it. An operation reports a failure by throwing SecurityException (its code
// is the `code` of the SecurityException of Table 28); any other exception
// is reported as code 7 (internal). A refusal is not a failure: the check_*
// operations return false.
//
// The functions of one plugin are never called concurrently. The header
// needs C++11 and zerodds.h.

#ifndef ZERODDS_SECURITY_PLUGINS_HPP
#define ZERODDS_SECURITY_PLUGINS_HPP

#include <array>
#include <cstdint>
#include <cstring>
#include <exception>
#include <map>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "zerodds.h"
#include "zerodds_security_plugins.h"

namespace zerodds {
namespace security {

typedef std::array<std::uint8_t, 16> Guid;

// The `code` of a SecurityException (Table 28).
enum ErrorCode {
    kAuthenticationFailed = 1,
    kAccessDenied = 2,
    kCryptoFailed = 3,
    kInvalidConfiguration = 4,
    kBadArgument = 5,
    kNotImplemented = 6,
    kInternal = 7
};

// SecurityException (Table 28): thrown by an operation that fails.
class SecurityException : public std::exception {
public:
    SecurityException(int code, const std::string& message, int minor_code = 0)
        : code_(code), minor_code_(minor_code), message_(message) {}
    int code() const { return code_; }
    int minor_code() const { return minor_code_; }
    const char* what() const throw() { return message_.c_str(); }
    ~SecurityException() throw() {}

private:
    int code_;
    int minor_code_;
    std::string message_;
};

struct Property {
    Property() : propagate(false) {}
    Property(const std::string& n, const std::string& v, bool p = false)
        : name(n), value(v), propagate(p) {}
    std::string name;
    std::string value;
    bool propagate;
};

struct BinaryProperty {
    BinaryProperty() : propagate(false) {}
    std::string name;
    std::vector<std::uint8_t> value;
    bool propagate;
};

// DataHolder (Table 6): the form of every token.
struct DataHolder {
    std::string class_id;
    std::vector<Property> properties;
    std::vector<BinaryProperty> binary_properties;
};

typedef std::pair<std::string, std::string> Pair;

struct Masks {
    Masks() : supported_mask(0), required_mask(0) {}
    std::uint32_t supported_mask;
    std::uint32_t required_mask;
};

struct AlgorithmMasks {
    Masks trust_chain;
    Masks message_auth;
    Masks key_establishment;
};

struct ParticipantAlgorithmInfo {
    ParticipantAlgorithmInfo()
        : cipher_supported_mask(0),
          cipher_builtin_endpoints_required_mask(0),
          cipher_builtin_kx_endpoints_required_mask(0),
          cipher_user_endpoints_default_required_mask(0) {}
    Masks trust_chain;
    Masks message_auth;
    Masks shared_secret;
    std::uint32_t cipher_supported_mask;
    std::uint32_t cipher_builtin_endpoints_required_mask;
    std::uint32_t cipher_builtin_kx_endpoints_required_mask;
    std::uint32_t cipher_user_endpoints_default_required_mask;
};

struct ParticipantRequest {
    ParticipantRequest() : domain_id(0), has_identity_algorithms(false) {
        participant_guid.fill(0);
    }
    std::uint32_t domain_id;
    std::string domain_tag;
    Guid participant_guid;
    std::vector<Property> properties;
    bool has_identity_algorithms;
    AlgorithmMasks identity_algorithms;
};

struct TopicRequest {
    TopicRequest() : domain_id(0) {}
    std::uint32_t domain_id;
    std::string topic_name;
    std::vector<Pair> qos;
};

struct EndpointRequest {
    EndpointRequest() : domain_id(0) {}
    std::uint32_t domain_id;
    std::string topic_name;
    std::vector<std::string> partitions;
    std::vector<Pair> data_tags;
    std::vector<Pair> qos;
};

struct ParticipantConfig {
    ParticipantConfig()
        : allow_unauthenticated_participants(false),
          is_access_protected(false),
          is_rtps_axk_protected(false),
          is_rtps_psk_protected(false),
          is_discovery_protected(false),
          is_liveliness_protected(false),
          is_key_revision_enabled(false),
          plugin_participant_attributes(0) {}
    bool allow_unauthenticated_participants;
    bool is_access_protected;
    bool is_rtps_axk_protected;
    bool is_rtps_psk_protected;
    bool is_discovery_protected;
    bool is_liveliness_protected;
    bool is_key_revision_enabled;
    std::uint32_t plugin_participant_attributes;
    std::vector<Property> ac_participant_properties;
    ParticipantAlgorithmInfo algorithm_info;
};

struct TopicConfig {
    TopicConfig()
        : is_read_protected(false),
          is_write_protected(false),
          is_discovery_protected(false),
          is_liveliness_protected(false) {}
    bool is_read_protected;
    bool is_write_protected;
    bool is_discovery_protected;
    bool is_liveliness_protected;
};

struct EndpointConfig {
    EndpointConfig()
        : is_submessage_protected(false),
          is_payload_protected(false),
          is_key_protected(false),
          plugin_endpoint_attributes(0),
          cipher_required_mask(0),
          cipher_supported_mask(0) {}
    TopicConfig topic;
    bool is_submessage_protected;
    bool is_payload_protected;
    bool is_key_protected;
    std::uint32_t plugin_endpoint_attributes;
    std::vector<Property> ac_endpoint_properties;
    std::uint32_t cipher_required_mask;
    std::uint32_t cipher_supported_mask;
};

enum RemoteReaderDecision { kDeny = 0, kPermit = 1, kRelayOnly = 2 };
enum RelayRole { kRelayReader = 0, kRelayWriter = 1 };

// AccessControlListener (9.4.2.10): the runtime gives it to the plugin with
// set_listener. The plugin calls on_revoke_permissions when it revokes the
// permissions behind a handle.
class AccessControlListener {
public:
    virtual ~AccessControlListener() {}
    virtual bool on_revoke_permissions(const std::string& plugin, std::uint64_t handle) = 0;
};

// AccessControl plugin (9.4.2.9, Table 39). Handles are 64-bit numbers, 0 is
// HandleNIL.
class AccessControlPlugin {
public:
    virtual ~AccessControlPlugin() {}
    virtual std::string plugin_class_id() const = 0;

    virtual std::uint64_t validate_local_permissions(std::uint64_t identity,
                                                     const ParticipantRequest& participant) = 0;
    virtual std::uint64_t validate_remote_permissions(std::uint64_t local_identity,
                                                      std::uint64_t remote_identity,
                                                      const DataHolder& remote_permissions_token,
                                                      const DataHolder& remote_credential_token) = 0;

    virtual bool check_create_participant(std::uint64_t permissions,
                                          const ParticipantRequest& participant) = 0;
    virtual bool check_create_datawriter(std::uint64_t permissions,
                                         const EndpointRequest& request) = 0;
    virtual bool check_create_datareader(std::uint64_t permissions,
                                         const EndpointRequest& request) = 0;
    virtual bool check_create_topic(std::uint64_t permissions, const TopicRequest& request) = 0;
    virtual bool check_local_datawriter_register_instance(std::uint64_t permissions,
                                                          const Guid& writer,
                                                          const std::vector<std::uint8_t>& key) = 0;
    virtual bool check_local_datawriter_dispose_instance(std::uint64_t permissions,
                                                         const Guid& writer,
                                                         const std::vector<std::uint8_t>& key) = 0;
    virtual bool check_remote_participant(std::uint64_t permissions,
                                          const ParticipantRequest& participant) = 0;
    virtual bool check_remote_datawriter(std::uint64_t permissions,
                                         const EndpointRequest& request) = 0;
    virtual RemoteReaderDecision check_remote_datareader(std::uint64_t permissions,
                                                         const EndpointRequest& request) = 0;
    virtual bool check_remote_topic(std::uint64_t permissions, const TopicRequest& request) = 0;
    virtual bool check_local_datawriter_match(std::uint64_t writer_permissions,
                                              std::uint64_t reader_permissions,
                                              const EndpointRequest& publication,
                                              const EndpointRequest& subscription) = 0;
    virtual bool check_local_datareader_match(std::uint64_t writer_permissions,
                                              std::uint64_t reader_permissions,
                                              const EndpointRequest& subscription,
                                              const EndpointRequest& publication) = 0;
    virtual bool check_remote_datawriter_register_instance(
        std::uint64_t permissions, const Guid& reader, const Guid& publication,
        const std::vector<std::uint8_t>& key) = 0;
    virtual bool check_remote_datawriter_dispose_instance(
        std::uint64_t permissions, const Guid& reader, const Guid& publication,
        const std::vector<std::uint8_t>& key) = 0;
    virtual bool check_relay(std::uint64_t permissions, RelayRole role,
                             const EndpointRequest& request) = 0;

    virtual DataHolder get_permissions_token(std::uint64_t handle) = 0;
    virtual DataHolder get_permissions_credential_token(std::uint64_t handle) = 0;
    // The listener belongs to the runtime and lives as long as the plugin.
    virtual void set_listener(AccessControlListener* listener) = 0;
    virtual void return_permissions_token(const DataHolder& token) = 0;
    virtual void return_permissions_credential_token(const DataHolder& token) = 0;

    virtual ParticipantConfig get_participant_security_config(std::uint64_t permissions) = 0;
    virtual TopicConfig get_topic_security_config(std::uint64_t permissions,
                                                  const std::string& topic_name) = 0;
    virtual EndpointConfig get_datawriter_security_config(std::uint64_t permissions,
                                                          const EndpointRequest& request) = 0;
    virtual EndpointConfig get_datareader_security_config(std::uint64_t permissions,
                                                          const EndpointRequest& request) = 0;
    virtual void return_participant_security_config(const ParticipantConfig& config) = 0;
    virtual void return_topic_security_config(const TopicConfig& config) = 0;
    virtual void return_datawriter_security_config(const EndpointConfig& config) = 0;
    virtual void return_datareader_security_config(const EndpointConfig& config) = 0;
};

// The levels of LogOptions (9.6.2.1.1).
enum LogLevel {
    kFatalLevel = 0,
    kSevereLevel = 1,
    kErrorLevel = 2,
    kWarningLevel = 3,
    kNoticeLevel = 4,
    kInfoLevel = 5,
    kDebugLevel = 6,
    kTraceLevel = 7
};

struct LogOptions {
    LogOptions() : log_level(kWarningLevel), has_log_file(false), distribute(false) {}
    int log_level;
    bool has_log_file;
    std::string log_file;
    bool distribute;
};

// LoggerListener (9.6.2.3): the plugin calls it for every message it logs.
class LoggerListener {
public:
    virtual ~LoggerListener() {}
    virtual int log_message(int level, const Guid& participant, const std::string& category,
                            const std::string& message) = 0;
};

// Logging plugin (9.6.2.2, Table 46).
class LoggingPlugin {
public:
    virtual ~LoggingPlugin() {}
    virtual std::string plugin_class_id() const = 0;
    virtual void set_log_options(const LogOptions& options) = 0;
    virtual void enable_logging() = 0;
    // `listener` is 0 for none; it lives until the next call or the end of
    // the plugin.
    virtual void set_listener(LoggerListener* listener) = 0;
    virtual void log(int level, const Guid& participant, const std::string& category,
                     const std::string& message) = 0;
};

struct DataTag {
    DataTag() {}
    DataTag(const std::string& n, const std::string& v) : name(n), value(v) {}
    std::string name;
    std::string value;
};

// DataTagging plugin (9.7).
class DataTaggingPlugin {
public:
    virtual ~DataTaggingPlugin() {}
    virtual std::string plugin_class_id() const = 0;
    virtual void set_tags(const Guid& endpoint, const std::vector<DataTag>& tags) = 0;
    virtual std::vector<DataTag> get_tags(const Guid& endpoint) = 0;
};


// ---------------------------------------------------------------------------
// Authentication and Cryptographic
// ---------------------------------------------------------------------------

typedef std::vector<std::uint8_t> Bytes;

// ValidationResult (9.3.2.11).
enum ValidationResult {
    kValidationOk = 0,
    kValidationFailed = 1,
    kValidationPendingRetry = 2,
    kValidationPendingHandshakeRequest = 3,
    kValidationPendingHandshakeMessage = 4,
    kValidationOkFinalMessage = 5
};

struct LocalIdentityValidation {
    LocalIdentityValidation() : result(kValidationOk), identity(0) { adjusted_guid.fill(0); }
    int result;
    std::uint64_t identity;
    Guid adjusted_guid;
};

struct RemoteIdentityValidation {
    RemoteIdentityValidation() : result(kValidationOk), identity(0), has_local_auth_request(false) {}
    int result;
    std::uint64_t identity;
    bool has_local_auth_request;
    Bytes local_auth_request;
};

// What a step of the handshake gives.
enum HandshakeKind {
    kHandshakeSendMessage = 0,
    kHandshakeSendFinalMessage = 1,
    kHandshakeComplete = 2,
    kHandshakeWaitingForPeer = 3,
    kHandshakePendingRetry = 4
};

struct HandshakeStep {
    HandshakeStep() : kind(kHandshakeWaitingForPeer), secret(0) {}
    int kind;
    Bytes token;
    std::uint64_t secret;
};

// AuthenticationListener (9.3.2.12): the runtime gives it to the plugin.
class AuthenticationListener {
public:
    virtual ~AuthenticationListener() {}
    // Returns 0 for success.
    virtual int on_revoke_identity(std::uint64_t handle) = 0;
    virtual void on_status_changed(std::uint64_t handle, int status_kind) = 0;
};

// Authentication plugin (9.3.2.11, Table 29). A token is the CDR of a
// DataHolder. A token an operation returns is given back with the matching
// return_* operation after the runtime copied it.
class AuthenticationPlugin {
public:
    virtual ~AuthenticationPlugin() {}
    virtual std::string plugin_class_id() const = 0;

    virtual LocalIdentityValidation validate_local_identity(
        std::uint32_t domain_id, const std::vector<Property>& properties,
        const std::vector<BinaryProperty>& binary_properties,
        const Guid& candidate_participant_guid) = 0;
    virtual Bytes get_identity_token(std::uint64_t identity) = 0;
    virtual Bytes get_identity_status_token(std::uint64_t identity) = 0;
    virtual AlgorithmMasks set_participant_security_config(std::uint64_t identity,
                                                           const AlgorithmMasks& config) = 0;
    virtual void set_permissions_credential_and_token(std::uint64_t identity,
                                                      const Bytes& credential_token,
                                                      const Bytes& permissions_token) = 0;
    // `remote_auth_request_token` is 0 for none.
    virtual RemoteIdentityValidation validate_remote_identity(
        std::uint64_t local_identity, const Guid& remote_participant_guid,
        const Bytes& remote_identity_token, const Bytes* remote_auth_request_token) = 0;
    virtual HandshakeStep begin_handshake_request(std::uint64_t initiator, std::uint64_t replier,
                                                  const Bytes& serialized_local_participant_data,
                                                  std::uint64_t* handshake) = 0;
    virtual HandshakeStep begin_handshake_reply(std::uint64_t replier, std::uint64_t initiator,
                                                const Bytes& request_token,
                                                const Bytes& serialized_local_participant_data,
                                                std::uint64_t* handshake) = 0;
    virtual HandshakeStep process_handshake(std::uint64_t handshake, const Bytes& token) = 0;
    virtual std::uint64_t get_shared_secret(std::uint64_t handshake) = 0;
    virtual Bytes get_authenticated_peer_credential_token(std::uint64_t handshake) = 0;
    // `listener` is 0 for none; it lives until the next call or the end of
    // the plugin.
    virtual void set_listener(AuthenticationListener* listener) = 0;
    virtual void return_identity_token(const Bytes& token) = 0;
    virtual void return_identity_status_token(const Bytes& token) = 0;
    virtual void return_authenticated_peer_credential_token(const Bytes& token) = 0;
    virtual void return_handshake_handle(std::uint64_t handshake) = 0;
    virtual void return_identity_handle(std::uint64_t identity) = 0;
    virtual void return_sharedsecret_handle(std::uint64_t secret) = 0;

    // The PermissionsToken the participant announces (empty for none).
    virtual Bytes get_permissions_token() { return Bytes(); }
    // The octets of a SharedSecret, where the builtin Cryptographic plugin is
    // used with this plugin; false for an unknown handle.
    virtual bool get_shared_secret_bytes(std::uint64_t, Bytes*) { return false; }
    virtual bool get_shared_secret_challenges(std::uint64_t, std::uint8_t*, std::uint8_t*) {
        return false;
    }
};

struct EndpointAlgorithmInfo {
    EndpointAlgorithmInfo() : required_mask(0), supported_mask(0) {}
    std::uint32_t required_mask;
    std::uint32_t supported_mask;
};

struct EncodedPayload {
    Bytes encoded;
    Bytes extra_inline_qos;
};

// SecureSubmessageCategory (9.5.1.10.5).
enum SubmessageCategory {
    kSubmessageInfo = 0,
    kSubmessageDatawriter = 1,
    kSubmessageDatareader = 2
};

struct PreprocessedSubmessage {
    PreprocessedSubmessage() : datawriter(0), datareader(0), category(kSubmessageInfo) {}
    std::uint64_t datawriter;
    std::uint64_t datareader;
    int category;
};

// Cryptographic plugin (9.5.1, Tables 42 to 44): CryptoKeyFactory,
// CryptoKeyExchange and CryptoTransform in one class. A `bool` result that is
// false is the answer "none" / "does not apply" (FALSE without an exception);
// a failure throws SecurityException.
class CryptographicPlugin {
public:
    virtual ~CryptographicPlugin() {}

    virtual std::uint64_t register_local_participant(std::uint64_t identity,
                                                     std::uint64_t permissions,
                                                     const std::vector<Property>& properties,
                                                     const ParticipantConfig& config,
                                                     ParticipantAlgorithmInfo* info) = 0;
    virtual std::uint64_t register_matched_remote_participant(std::uint64_t local_participant,
                                                              std::uint64_t remote_identity,
                                                              std::uint64_t remote_permissions,
                                                              std::uint64_t shared_secret) = 0;
    virtual std::uint64_t register_local_datawriter(std::uint64_t participant,
                                                    const std::vector<Property>& properties,
                                                    const EndpointConfig& config,
                                                    const Guid& endpoint_guid,
                                                    EndpointAlgorithmInfo* info) = 0;
    virtual std::uint64_t register_matched_remote_datareader(std::uint64_t local_datawriter,
                                                             std::uint64_t remote_participant,
                                                             std::uint64_t shared_secret,
                                                             bool relay_only) = 0;
    virtual std::uint64_t register_local_datareader(std::uint64_t participant,
                                                    const std::vector<Property>& properties,
                                                    const EndpointConfig& config,
                                                    const Guid& endpoint_guid,
                                                    EndpointAlgorithmInfo* info) = 0;
    virtual std::uint64_t register_matched_remote_datawriter(std::uint64_t local_datareader,
                                                             std::uint64_t remote_participant,
                                                             std::uint64_t shared_secret) = 0;
    virtual std::int32_t revise_local_entity_keys(std::uint64_t participant) = 0;
    virtual bool activate_key_revision(std::uint64_t participant, std::int32_t revision) = 0;
    virtual bool unregister_participant(std::uint64_t handle) = 0;
    virtual bool unregister_datawriter(std::uint64_t handle) = 0;
    virtual bool unregister_datareader(std::uint64_t handle) = 0;

    // The create_* operations return false for "no tokens".
    virtual bool create_local_participant_crypto_tokens(std::uint64_t local_participant,
                                                        std::uint64_t remote_participant,
                                                        std::int32_t key_revision,
                                                        std::vector<DataHolder>* tokens) = 0;
    virtual bool set_remote_participant_crypto_tokens(std::uint64_t local_participant,
                                                      std::uint64_t remote_participant,
                                                      const std::vector<DataHolder>& tokens) = 0;
    virtual bool create_local_datawriter_crypto_tokens(std::uint64_t local_datawriter,
                                                       std::uint64_t remote_datareader,
                                                       std::int32_t key_revision,
                                                       std::vector<DataHolder>* tokens) = 0;
    virtual bool set_remote_datawriter_crypto_tokens(std::uint64_t local_datareader,
                                                     std::uint64_t remote_datawriter,
                                                     const std::vector<DataHolder>& tokens) = 0;
    virtual bool create_local_datareader_crypto_tokens(std::uint64_t local_datareader,
                                                       std::uint64_t remote_datawriter,
                                                       std::int32_t key_revision,
                                                       std::vector<DataHolder>* tokens) = 0;
    virtual bool set_remote_datareader_crypto_tokens(std::uint64_t local_datawriter,
                                                     std::uint64_t remote_datareader,
                                                     const std::vector<DataHolder>& tokens) = 0;
    virtual bool return_crypto_tokens(const std::vector<DataHolder>& tokens) = 0;

    virtual bool encode_serialized_payload(const Bytes& plain, std::uint64_t sending_datawriter,
                                           EncodedPayload* out) = 0;
    // `*index` is the receiving_datareader_crypto_list_index, in and out.
    virtual bool encode_datawriter_submessage(const Bytes& plain, std::uint64_t sending_datawriter,
                                              const std::vector<std::uint64_t>& receiving_datareaders,
                                              std::size_t* index, Bytes* out) = 0;
    virtual bool encode_datareader_submessage(const Bytes& plain, std::uint64_t sending_datareader,
                                              const std::vector<std::uint64_t>& receiving_datawriters,
                                              Bytes* out) = 0;
    virtual bool encode_rtps_message(const Bytes& plain, std::uint64_t sending_participant,
                                     const std::vector<std::uint64_t>& receiving_participants,
                                     std::size_t* index, bool transform_with_psk, Bytes* out) = 0;
    virtual bool decode_rtps_message(const Bytes& encoded, std::uint64_t receiving_participant,
                                     std::uint64_t sending_participant, Bytes* out) = 0;
    virtual bool preprocess_secure_submsg(const Bytes& encoded_submessage,
                                          std::uint64_t receiving_participant,
                                          std::uint64_t sending_participant,
                                          PreprocessedSubmessage* out) = 0;
    virtual bool decode_datawriter_submessage(const Bytes& encoded_submessage,
                                              std::uint64_t receiving_datareader,
                                              std::uint64_t sending_datawriter, Bytes* out) = 0;
    virtual bool decode_datareader_submessage(const Bytes& encoded_submessage,
                                              std::uint64_t receiving_datawriter,
                                              std::uint64_t sending_datareader, Bytes* out) = 0;
    virtual bool decode_serialized_payload(const Bytes& encoded, const Bytes& inline_qos,
                                           std::uint64_t receiving_datareader,
                                           std::uint64_t sending_datawriter, Bytes* out) = 0;
};

// ---------------------------------------------------------------------------
// The adapters between the C structs and these classes
// ---------------------------------------------------------------------------

namespace detail {

inline std::string text(const char* s) { return s ? std::string(s) : std::string(); }

inline Guid guid_of(const std::uint8_t* g) {
    Guid out;
    std::memcpy(out.data(), g, 16);
    return out;
}

inline std::vector<std::uint8_t> bytes_of(const std::uint8_t* data, std::size_t len) {
    return data && len ? std::vector<std::uint8_t>(data, data + len)
                       : std::vector<std::uint8_t>();
}

inline std::vector<Property> properties_of(const ZeroDdsPluginProperty* p, std::size_t n) {
    std::vector<Property> out;
    for (std::size_t i = 0; i < n; ++i) {
        out.push_back(Property(text(p[i].name), text(p[i].value), p[i].propagate));
    }
    return out;
}

inline std::vector<Pair> pairs_of(const ZeroDdsPluginPair* p, std::size_t n) {
    std::vector<Pair> out;
    for (std::size_t i = 0; i < n; ++i) {
        out.push_back(Pair(text(p[i].name), text(p[i].value)));
    }
    return out;
}

inline Masks masks_of(const ZeroDdsPluginMasks& m) {
    Masks out;
    out.supported_mask = m.supported_mask;
    out.required_mask = m.required_mask;
    return out;
}

inline ZeroDdsPluginMasks masks_to(const Masks& m) {
    ZeroDdsPluginMasks out;
    out.supported_mask = m.supported_mask;
    out.required_mask = m.required_mask;
    return out;
}

inline DataHolder holder_of(const ZeroDdsPluginDataHolder& h) {
    DataHolder out;
    out.class_id = text(h.class_id);
    out.properties = properties_of(h.properties, h.n_properties);
    for (std::size_t i = 0; i < h.n_binary_properties; ++i) {
        BinaryProperty b;
        b.name = text(h.binary_properties[i].name);
        b.value = bytes_of(h.binary_properties[i].value, h.binary_properties[i].value_len);
        b.propagate = h.binary_properties[i].propagate;
        out.binary_properties.push_back(b);
    }
    return out;
}

inline ParticipantRequest participant_request_of(const ZeroDdsPluginParticipantRequest& r) {
    ParticipantRequest out;
    out.domain_id = r.domain_id;
    out.domain_tag = text(r.domain_tag);
    out.participant_guid = guid_of(r.participant_guid);
    out.properties = properties_of(r.properties, r.n_properties);
    out.has_identity_algorithms = r.has_identity_algorithms;
    out.identity_algorithms.trust_chain = masks_of(r.identity_algorithms.trust_chain);
    out.identity_algorithms.message_auth = masks_of(r.identity_algorithms.message_auth);
    out.identity_algorithms.key_establishment = masks_of(r.identity_algorithms.key_establishment);
    return out;
}

inline TopicRequest topic_request_of(const ZeroDdsPluginTopicRequest& r) {
    TopicRequest out;
    out.domain_id = r.domain_id;
    out.topic_name = text(r.topic_name);
    out.qos = pairs_of(r.qos, r.n_qos);
    return out;
}

inline EndpointRequest endpoint_request_of(const ZeroDdsPluginEndpointRequest& r) {
    EndpointRequest out;
    out.domain_id = r.domain_id;
    out.topic_name = text(r.topic_name);
    for (std::size_t i = 0; i < r.n_partitions; ++i) {
        out.partitions.push_back(text(r.partitions[i]));
    }
    out.data_tags = pairs_of(r.data_tags, r.n_data_tags);
    out.qos = pairs_of(r.qos, r.n_qos);
    return out;
}

inline bool same(const std::vector<Property>& a, const std::vector<Property>& b) {
    if (a.size() != b.size()) return false;
    for (std::size_t i = 0; i < a.size(); ++i) {
        if (a[i].name != b[i].name || a[i].value != b[i].value || a[i].propagate != b[i].propagate)
            return false;
    }
    return true;
}

inline bool same(const Masks& a, const Masks& b) {
    return a.supported_mask == b.supported_mask && a.required_mask == b.required_mask;
}

inline bool same(const ParticipantConfig& a, const ParticipantConfig& b) {
    const ParticipantAlgorithmInfo& x = a.algorithm_info;
    const ParticipantAlgorithmInfo& y = b.algorithm_info;
    return a.allow_unauthenticated_participants == b.allow_unauthenticated_participants &&
           a.is_access_protected == b.is_access_protected &&
           a.is_rtps_axk_protected == b.is_rtps_axk_protected &&
           a.is_rtps_psk_protected == b.is_rtps_psk_protected &&
           a.is_discovery_protected == b.is_discovery_protected &&
           a.is_liveliness_protected == b.is_liveliness_protected &&
           a.is_key_revision_enabled == b.is_key_revision_enabled &&
           a.plugin_participant_attributes == b.plugin_participant_attributes &&
           same(a.ac_participant_properties, b.ac_participant_properties) &&
           same(x.trust_chain, y.trust_chain) && same(x.message_auth, y.message_auth) &&
           same(x.shared_secret, y.shared_secret) &&
           x.cipher_supported_mask == y.cipher_supported_mask &&
           x.cipher_builtin_endpoints_required_mask == y.cipher_builtin_endpoints_required_mask &&
           x.cipher_builtin_kx_endpoints_required_mask ==
               y.cipher_builtin_kx_endpoints_required_mask &&
           x.cipher_user_endpoints_default_required_mask ==
               y.cipher_user_endpoints_default_required_mask;
}

inline bool same(const EndpointConfig& a, const EndpointConfig& b) {
    return a.topic.is_read_protected == b.topic.is_read_protected &&
           a.topic.is_write_protected == b.topic.is_write_protected &&
           a.topic.is_discovery_protected == b.topic.is_discovery_protected &&
           a.topic.is_liveliness_protected == b.topic.is_liveliness_protected &&
           a.is_submessage_protected == b.is_submessage_protected &&
           a.is_payload_protected == b.is_payload_protected &&
           a.is_key_protected == b.is_key_protected &&
           a.plugin_endpoint_attributes == b.plugin_endpoint_attributes &&
           same(a.ac_endpoint_properties, b.ac_endpoint_properties) &&
           a.cipher_required_mask == b.cipher_required_mask &&
           a.cipher_supported_mask == b.cipher_supported_mask;
}

// What an operation handed out and the runtime has not given back: the C view
// and the memory behind it.
struct IssuedHolder {
    DataHolder value;
    std::vector<ZeroDdsPluginProperty> properties;
    std::vector<ZeroDdsPluginBinaryProperty> binary;
    ZeroDdsPluginDataHolder raw;

    explicit IssuedHolder(const DataHolder& v) : value(v) {
        for (std::size_t i = 0; i < value.properties.size(); ++i) {
            ZeroDdsPluginProperty p;
            p.name = value.properties[i].name.c_str();
            p.value = value.properties[i].value.c_str();
            p.propagate = value.properties[i].propagate;
            properties.push_back(p);
        }
        for (std::size_t i = 0; i < value.binary_properties.size(); ++i) {
            ZeroDdsPluginBinaryProperty b;
            b.name = value.binary_properties[i].name.c_str();
            b.value = value.binary_properties[i].value.data();
            b.value_len = value.binary_properties[i].value.size();
            b.propagate = value.binary_properties[i].propagate;
            binary.push_back(b);
        }
        raw.class_id = value.class_id.c_str();
        raw.properties = properties.empty() ? 0 : properties.data();
        raw.n_properties = properties.size();
        raw.binary_properties = binary.empty() ? 0 : binary.data();
        raw.n_binary_properties = binary.size();
    }

private:
    IssuedHolder(const IssuedHolder&);
    IssuedHolder& operator=(const IssuedHolder&);
};

inline std::vector<ZeroDdsPluginProperty> properties_to(const std::vector<Property>& in) {
    std::vector<ZeroDdsPluginProperty> out;
    for (std::size_t i = 0; i < in.size(); ++i) {
        ZeroDdsPluginProperty p;
        p.name = in[i].name.c_str();
        p.value = in[i].value.c_str();
        p.propagate = in[i].propagate;
        out.push_back(p);
    }
    return out;
}

struct IssuedParticipantConfig {
    ParticipantConfig value;
    std::vector<ZeroDdsPluginProperty> properties;
    ZeroDdsPluginParticipantConfig raw;

    explicit IssuedParticipantConfig(const ParticipantConfig& v) : value(v) {
        properties = properties_to(value.ac_participant_properties);
        raw.allow_unauthenticated_participants = value.allow_unauthenticated_participants;
        raw.is_access_protected = value.is_access_protected;
        raw.is_rtps_axk_protected = value.is_rtps_axk_protected;
        raw.is_rtps_psk_protected = value.is_rtps_psk_protected;
        raw.is_discovery_protected = value.is_discovery_protected;
        raw.is_liveliness_protected = value.is_liveliness_protected;
        raw.is_key_revision_enabled = value.is_key_revision_enabled;
        raw.plugin_participant_attributes = value.plugin_participant_attributes;
        raw.ac_participant_properties = properties.empty() ? 0 : properties.data();
        raw.n_ac_participant_properties = properties.size();
        const ParticipantAlgorithmInfo& a = value.algorithm_info;
        raw.algorithm_info.trust_chain = masks_to(a.trust_chain);
        raw.algorithm_info.message_auth = masks_to(a.message_auth);
        raw.algorithm_info.shared_secret = masks_to(a.shared_secret);
        raw.algorithm_info.cipher_supported_mask = a.cipher_supported_mask;
        raw.algorithm_info.cipher_builtin_endpoints_required_mask =
            a.cipher_builtin_endpoints_required_mask;
        raw.algorithm_info.cipher_builtin_kx_endpoints_required_mask =
            a.cipher_builtin_kx_endpoints_required_mask;
        raw.algorithm_info.cipher_user_endpoints_default_required_mask =
            a.cipher_user_endpoints_default_required_mask;
    }

private:
    IssuedParticipantConfig(const IssuedParticipantConfig&);
    IssuedParticipantConfig& operator=(const IssuedParticipantConfig&);
};

inline ZeroDdsPluginTopicConfig topic_config_to(const TopicConfig& c) {
    ZeroDdsPluginTopicConfig out;
    out.is_read_protected = c.is_read_protected;
    out.is_write_protected = c.is_write_protected;
    out.is_discovery_protected = c.is_discovery_protected;
    out.is_liveliness_protected = c.is_liveliness_protected;
    return out;
}

inline TopicConfig topic_config_of(const ZeroDdsPluginTopicConfig& c) {
    TopicConfig out;
    out.is_read_protected = c.is_read_protected;
    out.is_write_protected = c.is_write_protected;
    out.is_discovery_protected = c.is_discovery_protected;
    out.is_liveliness_protected = c.is_liveliness_protected;
    return out;
}

struct IssuedEndpointConfig {
    EndpointConfig value;
    std::vector<ZeroDdsPluginProperty> properties;
    ZeroDdsPluginEndpointConfig raw;

    explicit IssuedEndpointConfig(const EndpointConfig& v) : value(v) {
        properties = properties_to(value.ac_endpoint_properties);
        raw.topic = topic_config_to(value.topic);
        raw.is_submessage_protected = value.is_submessage_protected;
        raw.is_payload_protected = value.is_payload_protected;
        raw.is_key_protected = value.is_key_protected;
        raw.plugin_endpoint_attributes = value.plugin_endpoint_attributes;
        raw.ac_endpoint_properties = properties.empty() ? 0 : properties.data();
        raw.n_ac_endpoint_properties = properties.size();
        raw.cipher_required_mask = value.cipher_required_mask;
        raw.cipher_supported_mask = value.cipher_supported_mask;
    }

private:
    IssuedEndpointConfig(const IssuedEndpointConfig&);
    IssuedEndpointConfig& operator=(const IssuedEndpointConfig&);
};

inline ParticipantConfig participant_config_of(const ZeroDdsPluginParticipantConfig& c) {
    ParticipantConfig out;
    out.allow_unauthenticated_participants = c.allow_unauthenticated_participants;
    out.is_access_protected = c.is_access_protected;
    out.is_rtps_axk_protected = c.is_rtps_axk_protected;
    out.is_rtps_psk_protected = c.is_rtps_psk_protected;
    out.is_discovery_protected = c.is_discovery_protected;
    out.is_liveliness_protected = c.is_liveliness_protected;
    out.is_key_revision_enabled = c.is_key_revision_enabled;
    out.plugin_participant_attributes = c.plugin_participant_attributes;
    out.ac_participant_properties =
        properties_of(c.ac_participant_properties, c.n_ac_participant_properties);
    out.algorithm_info.trust_chain = masks_of(c.algorithm_info.trust_chain);
    out.algorithm_info.message_auth = masks_of(c.algorithm_info.message_auth);
    out.algorithm_info.shared_secret = masks_of(c.algorithm_info.shared_secret);
    out.algorithm_info.cipher_supported_mask = c.algorithm_info.cipher_supported_mask;
    out.algorithm_info.cipher_builtin_endpoints_required_mask =
        c.algorithm_info.cipher_builtin_endpoints_required_mask;
    out.algorithm_info.cipher_builtin_kx_endpoints_required_mask =
        c.algorithm_info.cipher_builtin_kx_endpoints_required_mask;
    out.algorithm_info.cipher_user_endpoints_default_required_mask =
        c.algorithm_info.cipher_user_endpoints_default_required_mask;
    return out;
}

inline EndpointConfig endpoint_config_of(const ZeroDdsPluginEndpointConfig& c) {
    EndpointConfig out;
    out.topic = topic_config_of(c.topic);
    out.is_submessage_protected = c.is_submessage_protected;
    out.is_payload_protected = c.is_payload_protected;
    out.is_key_protected = c.is_key_protected;
    out.plugin_endpoint_attributes = c.plugin_endpoint_attributes;
    out.ac_endpoint_properties =
        properties_of(c.ac_endpoint_properties, c.n_ac_endpoint_properties);
    out.cipher_required_mask = c.cipher_required_mask;
    out.cipher_supported_mask = c.cipher_supported_mask;
    return out;
}

// Runs `body` and turns an exception into the status and the error struct.
class Guard {
public:
    Guard(std::string* message, ZeroDdsPluginError* err) : message_(message), err_(err) {}

    template <typename F>
    ZeroDdsPluginStatus run(F body) {
        try {
            body();
            return 0;
        } catch (const SecurityException& e) {
            return report(e.code(), e.what(), e.minor_code());
        } catch (const std::exception& e) {
            return report(kInternal, e.what(), 0);
        } catch (...) {
            return report(kInternal, "the plugin threw an exception", 0);
        }
    }

private:
    ZeroDdsPluginStatus report(int code, const char* what, int minor_code) {
        *message_ = what;
        if (err_) {
            err_->message = message_->c_str();
            err_->minor_code = minor_code;
        }
        return code;
    }
    std::string* message_;
    ZeroDdsPluginError* err_;
};

class AccessControlAdapter {
public:
    explicit AccessControlAdapter(std::unique_ptr<AccessControlPlugin> p)
        : impl(std::move(p)), class_id(impl->plugin_class_id()) {
        listener_context.owner = this;
    }

    std::unique_ptr<AccessControlPlugin> impl;
    std::string class_id;
    std::string message;
    std::vector<std::unique_ptr<IssuedHolder> > permissions_tokens;
    std::vector<std::unique_ptr<IssuedHolder> > credential_tokens;
    std::vector<std::unique_ptr<IssuedParticipantConfig> > participant_configs;
    std::vector<std::unique_ptr<IssuedEndpointConfig> > writer_configs;
    std::vector<std::unique_ptr<IssuedEndpointConfig> > reader_configs;

    // The listener of the runtime, seen from the plugin.
    struct ListenerProxy : public AccessControlListener {
        ListenerProxy() : owner(0), listener(0) {}
        bool on_revoke_permissions(const std::string& plugin, std::uint64_t handle) {
            return listener && listener->on_revoke_permissions
                       ? listener->on_revoke_permissions(listener->context, plugin.c_str(), handle)
                       : false;
        }
        AccessControlAdapter* owner;
        const ZeroDdsPluginAccessControlListener* listener;
    } listener_context;

    static AccessControlAdapter* self(void* user) {
        return static_cast<AccessControlAdapter*>(user);
    }

    template <typename Vec>
    static void give_back(Vec& list, const ZeroDdsPluginDataHolder* raw,
                          std::unique_ptr<IssuedHolder>* found) {
        for (typename Vec::iterator it = list.begin(); it != list.end(); ++it) {
            if (&(*it)->raw == raw || (*it)->raw.class_id == raw->class_id) {
                *found = std::move(*it);
                list.erase(it);
                return;
            }
        }
    }

    static const char* class_id_of(void* user) { return self(user)->class_id.c_str(); }

    static ZeroDdsPluginStatus validate_local(void* user, std::uint64_t identity,
                                              const ZeroDdsPluginParticipantRequest* p,
                                              std::uint64_t* out, ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = a->impl->validate_local_permissions(identity, participant_request_of(*p));
        });
    }

    static ZeroDdsPluginStatus validate_remote(void* user, std::uint64_t li, std::uint64_t ri,
                                               const ZeroDdsPluginDataHolder* pt,
                                               const ZeroDdsPluginDataHolder* ct,
                                               std::uint64_t* out, ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = a->impl->validate_remote_permissions(li, ri, holder_of(*pt), holder_of(*ct));
        });
    }

    static ZeroDdsPluginStatus create_participant(void* user, std::uint64_t perm,
                                                  const ZeroDdsPluginParticipantRequest* p,
                                                  bool* out, ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run(
            [&] { *out = a->impl->check_create_participant(perm, participant_request_of(*p)); });
    }

    static ZeroDdsPluginStatus remote_participant(void* user, std::uint64_t perm,
                                                  const ZeroDdsPluginParticipantRequest* p,
                                                  bool* out, ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run(
            [&] { *out = a->impl->check_remote_participant(perm, participant_request_of(*p)); });
    }

    static ZeroDdsPluginStatus create_datawriter(void* user, std::uint64_t perm,
                                                 const ZeroDdsPluginEndpointRequest* r, bool* out,
                                                 ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run(
            [&] { *out = a->impl->check_create_datawriter(perm, endpoint_request_of(*r)); });
    }

    static ZeroDdsPluginStatus create_datareader(void* user, std::uint64_t perm,
                                                 const ZeroDdsPluginEndpointRequest* r, bool* out,
                                                 ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run(
            [&] { *out = a->impl->check_create_datareader(perm, endpoint_request_of(*r)); });
    }

    static ZeroDdsPluginStatus create_topic(void* user, std::uint64_t perm,
                                            const ZeroDdsPluginTopicRequest* r, bool* out,
                                            ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run(
            [&] { *out = a->impl->check_create_topic(perm, topic_request_of(*r)); });
    }

    static ZeroDdsPluginStatus local_register(void* user, std::uint64_t perm,
                                              const std::uint8_t* writer, const std::uint8_t* key,
                                              std::size_t key_len, bool* out,
                                              ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = a->impl->check_local_datawriter_register_instance(perm, guid_of(writer),
                                                                     bytes_of(key, key_len));
        });
    }

    static ZeroDdsPluginStatus local_dispose(void* user, std::uint64_t perm,
                                             const std::uint8_t* writer, const std::uint8_t* key,
                                             std::size_t key_len, bool* out,
                                             ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = a->impl->check_local_datawriter_dispose_instance(perm, guid_of(writer),
                                                                    bytes_of(key, key_len));
        });
    }

    static ZeroDdsPluginStatus remote_datawriter(void* user, std::uint64_t perm,
                                                 const ZeroDdsPluginEndpointRequest* r, bool* out,
                                                 ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run(
            [&] { *out = a->impl->check_remote_datawriter(perm, endpoint_request_of(*r)); });
    }

    static ZeroDdsPluginStatus remote_datareader(void* user, std::uint64_t perm,
                                                 const ZeroDdsPluginEndpointRequest* r,
                                                 std::int32_t* out, ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = static_cast<std::int32_t>(
                a->impl->check_remote_datareader(perm, endpoint_request_of(*r)));
        });
    }

    static ZeroDdsPluginStatus remote_topic(void* user, std::uint64_t perm,
                                            const ZeroDdsPluginTopicRequest* r, bool* out,
                                            ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run(
            [&] { *out = a->impl->check_remote_topic(perm, topic_request_of(*r)); });
    }

    static ZeroDdsPluginStatus datawriter_match(void* user, std::uint64_t wp, std::uint64_t rp,
                                                const ZeroDdsPluginEndpointRequest* x,
                                                const ZeroDdsPluginEndpointRequest* y, bool* out,
                                                ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = a->impl->check_local_datawriter_match(wp, rp, endpoint_request_of(*x),
                                                         endpoint_request_of(*y));
        });
    }

    static ZeroDdsPluginStatus datareader_match(void* user, std::uint64_t wp, std::uint64_t rp,
                                                const ZeroDdsPluginEndpointRequest* x,
                                                const ZeroDdsPluginEndpointRequest* y, bool* out,
                                                ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = a->impl->check_local_datareader_match(wp, rp, endpoint_request_of(*x),
                                                         endpoint_request_of(*y));
        });
    }

    static ZeroDdsPluginStatus remote_register(void* user, std::uint64_t perm,
                                               const std::uint8_t* reader,
                                               const std::uint8_t* publication,
                                               const std::uint8_t* key, std::size_t key_len,
                                               bool* out, ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = a->impl->check_remote_datawriter_register_instance(
                perm, guid_of(reader), guid_of(publication), bytes_of(key, key_len));
        });
    }

    static ZeroDdsPluginStatus remote_dispose(void* user, std::uint64_t perm,
                                              const std::uint8_t* reader,
                                              const std::uint8_t* publication,
                                              const std::uint8_t* key, std::size_t key_len,
                                              bool* out, ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = a->impl->check_remote_datawriter_dispose_instance(
                perm, guid_of(reader), guid_of(publication), bytes_of(key, key_len));
        });
    }

    static ZeroDdsPluginStatus relay(void* user, std::uint64_t perm, std::int32_t role,
                                     const ZeroDdsPluginEndpointRequest* r, bool* out,
                                     ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = a->impl->check_relay(perm, role == kRelayWriter ? kRelayWriter : kRelayReader,
                                        endpoint_request_of(*r));
        });
    }

    static ZeroDdsPluginStatus token(void* user, std::uint64_t handle, ZeroDdsPluginDataHolder* out,
                                     ZeroDdsPluginError* err, bool credential) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            DataHolder v = credential ? a->impl->get_permissions_credential_token(handle)
                                      : a->impl->get_permissions_token(handle);
            std::unique_ptr<IssuedHolder> issued(new IssuedHolder(v));
            *out = issued->raw;
            (credential ? a->credential_tokens : a->permissions_tokens).push_back(std::move(issued));
        });
    }

    static ZeroDdsPluginStatus permissions_token(void* user, std::uint64_t h,
                                                 ZeroDdsPluginDataHolder* out,
                                                 ZeroDdsPluginError* err) {
        return token(user, h, out, err, false);
    }

    static ZeroDdsPluginStatus credential_token(void* user, std::uint64_t h,
                                                ZeroDdsPluginDataHolder* out,
                                                ZeroDdsPluginError* err) {
        return token(user, h, out, err, true);
    }

    static ZeroDdsPluginStatus set_listener_c(void* user,
                                              const ZeroDdsPluginAccessControlListener* l,
                                              ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            a->listener_context.listener = l;
            a->impl->set_listener(&a->listener_context);
        });
    }

    static ZeroDdsPluginStatus return_token(void* user, const ZeroDdsPluginDataHolder* t,
                                            ZeroDdsPluginError* err, bool credential) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            std::unique_ptr<IssuedHolder> found;
            give_back(credential ? a->credential_tokens : a->permissions_tokens, t, &found);
            if (!found) {
                throw SecurityException(kBadArgument, "the token was not issued by this plugin");
            }
            if (credential) {
                a->impl->return_permissions_credential_token(found->value);
            } else {
                a->impl->return_permissions_token(found->value);
            }
        });
    }

    static ZeroDdsPluginStatus return_permissions_token_c(void* user,
                                                          const ZeroDdsPluginDataHolder* t,
                                                          ZeroDdsPluginError* err) {
        return return_token(user, t, err, false);
    }

    static ZeroDdsPluginStatus return_credential_token_c(void* user,
                                                         const ZeroDdsPluginDataHolder* t,
                                                         ZeroDdsPluginError* err) {
        return return_token(user, t, err, true);
    }

    static ZeroDdsPluginStatus participant_config(void* user, std::uint64_t perm,
                                                  ZeroDdsPluginParticipantConfig* out,
                                                  ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            std::unique_ptr<IssuedParticipantConfig> issued(
                new IssuedParticipantConfig(a->impl->get_participant_security_config(perm)));
            *out = issued->raw;
            a->participant_configs.push_back(std::move(issued));
        });
    }

    static ZeroDdsPluginStatus topic_config(void* user, std::uint64_t perm, const char* topic,
                                            ZeroDdsPluginTopicConfig* out,
                                            ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = topic_config_to(a->impl->get_topic_security_config(perm, text(topic)));
        });
    }

    static ZeroDdsPluginStatus endpoint_config(void* user, std::uint64_t perm,
                                               const ZeroDdsPluginEndpointRequest* r,
                                               ZeroDdsPluginEndpointConfig* out,
                                               ZeroDdsPluginError* err, bool writer) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            EndpointRequest request = endpoint_request_of(*r);
            std::unique_ptr<IssuedEndpointConfig> issued(new IssuedEndpointConfig(
                writer ? a->impl->get_datawriter_security_config(perm, request)
                       : a->impl->get_datareader_security_config(perm, request)));
            *out = issued->raw;
            (writer ? a->writer_configs : a->reader_configs).push_back(std::move(issued));
        });
    }

    static ZeroDdsPluginStatus writer_config(void* u, std::uint64_t p,
                                             const ZeroDdsPluginEndpointRequest* r,
                                             ZeroDdsPluginEndpointConfig* o,
                                             ZeroDdsPluginError* e) {
        return endpoint_config(u, p, r, o, e, true);
    }

    static ZeroDdsPluginStatus reader_config(void* u, std::uint64_t p,
                                             const ZeroDdsPluginEndpointRequest* r,
                                             ZeroDdsPluginEndpointConfig* o,
                                             ZeroDdsPluginError* e) {
        return endpoint_config(u, p, r, o, e, false);
    }

    static ZeroDdsPluginStatus return_participant_config(void* user,
                                                         const ZeroDdsPluginParticipantConfig* c,
                                                         ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            ParticipantConfig value = participant_config_of(*c);
            for (std::vector<std::unique_ptr<IssuedParticipantConfig> >::iterator it =
                     a->participant_configs.begin();
                 it != a->participant_configs.end(); ++it) {
                if (same((*it)->value, value)) {
                    a->participant_configs.erase(it);
                    break;
                }
            }
            a->impl->return_participant_security_config(value);
        });
    }

    static ZeroDdsPluginStatus return_topic_config(void* user, const ZeroDdsPluginTopicConfig* c,
                                                   ZeroDdsPluginError* err) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run(
            [&] { a->impl->return_topic_security_config(topic_config_of(*c)); });
    }

    static ZeroDdsPluginStatus return_endpoint_config(void* user,
                                                      const ZeroDdsPluginEndpointConfig* c,
                                                      ZeroDdsPluginError* err, bool writer) {
        AccessControlAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            EndpointConfig value = endpoint_config_of(*c);
            std::vector<std::unique_ptr<IssuedEndpointConfig> >& list =
                writer ? a->writer_configs : a->reader_configs;
            for (std::vector<std::unique_ptr<IssuedEndpointConfig> >::iterator it = list.begin();
                 it != list.end(); ++it) {
                if (same((*it)->value, value)) {
                    list.erase(it);
                    break;
                }
            }
            if (writer) {
                a->impl->return_datawriter_security_config(value);
            } else {
                a->impl->return_datareader_security_config(value);
            }
        });
    }

    static ZeroDdsPluginStatus return_writer_config(void* u, const ZeroDdsPluginEndpointConfig* c,
                                                    ZeroDdsPluginError* e) {
        return return_endpoint_config(u, c, e, true);
    }

    static ZeroDdsPluginStatus return_reader_config(void* u, const ZeroDdsPluginEndpointConfig* c,
                                                    ZeroDdsPluginError* e) {
        return return_endpoint_config(u, c, e, false);
    }

    static void destroy(void* user) { delete self(user); }
};

}  // namespace detail

// Places `plugin` into the security configuration `cfg`. The runtime owns the
// object from the call on and deletes it when it lets go of it. Returns 0, or
// the status of zerodds_security_set_access_control_plugin (-1 for a NULL
// argument, -2 for a plugin the runtime refuses); the object is deleted then.
inline int set_access_control_plugin(zerodds_ZeroDdsSecurityConfig* cfg,
                                     std::unique_ptr<AccessControlPlugin> plugin) {
    using detail::AccessControlAdapter;
    AccessControlAdapter* adapter = new AccessControlAdapter(std::move(plugin));
    ZeroDdsPluginAccessControl table;
    std::memset(&table, 0, sizeof(table));
    table.abi_version = ZERODDS_PLUGIN_ABI_VERSION;
    table.user = adapter;
    table.plugin_class_id = AccessControlAdapter::class_id_of;
    table.validate_local_permissions = AccessControlAdapter::validate_local;
    table.validate_remote_permissions = AccessControlAdapter::validate_remote;
    table.check_create_participant = AccessControlAdapter::create_participant;
    table.check_create_datawriter = AccessControlAdapter::create_datawriter;
    table.check_create_datareader = AccessControlAdapter::create_datareader;
    table.check_create_topic = AccessControlAdapter::create_topic;
    table.check_local_datawriter_register_instance = AccessControlAdapter::local_register;
    table.check_local_datawriter_dispose_instance = AccessControlAdapter::local_dispose;
    table.check_remote_participant = AccessControlAdapter::remote_participant;
    table.check_remote_datawriter = AccessControlAdapter::remote_datawriter;
    table.check_remote_datareader = AccessControlAdapter::remote_datareader;
    table.check_remote_topic = AccessControlAdapter::remote_topic;
    table.check_local_datawriter_match = AccessControlAdapter::datawriter_match;
    table.check_local_datareader_match = AccessControlAdapter::datareader_match;
    table.check_remote_datawriter_register_instance = AccessControlAdapter::remote_register;
    table.check_remote_datawriter_dispose_instance = AccessControlAdapter::remote_dispose;
    table.check_relay = AccessControlAdapter::relay;
    table.get_permissions_token = AccessControlAdapter::permissions_token;
    table.get_permissions_credential_token = AccessControlAdapter::credential_token;
    table.set_listener = AccessControlAdapter::set_listener_c;
    table.return_permissions_token = AccessControlAdapter::return_permissions_token_c;
    table.return_permissions_credential_token = AccessControlAdapter::return_credential_token_c;
    table.get_participant_security_config = AccessControlAdapter::participant_config;
    table.get_topic_security_config = AccessControlAdapter::topic_config;
    table.get_datawriter_security_config = AccessControlAdapter::writer_config;
    table.get_datareader_security_config = AccessControlAdapter::reader_config;
    table.return_participant_security_config = AccessControlAdapter::return_participant_config;
    table.return_topic_security_config = AccessControlAdapter::return_topic_config;
    table.return_datawriter_security_config = AccessControlAdapter::return_writer_config;
    table.return_datareader_security_config = AccessControlAdapter::return_reader_config;
    table.destroy = AccessControlAdapter::destroy;
    int status = zerodds_security_set_access_control_plugin(cfg, &table);
    if (status != 0) {
        delete adapter;
    }
    return status;
}

namespace detail {

class LoggingAdapter {
public:
    explicit LoggingAdapter(std::unique_ptr<LoggingPlugin> p)
        : impl(std::move(p)), class_id(impl->plugin_class_id()) {
        proxy.listener = 0;
    }

    std::unique_ptr<LoggingPlugin> impl;
    std::string class_id;
    std::string message;

    struct ListenerProxy : public LoggerListener {
        ListenerProxy() : listener(0) {}
        int log_message(int level, const Guid& participant, const std::string& category,
                        const std::string& message) {
            return listener && listener->log_message
                       ? listener->log_message(listener->context, level, participant.data(),
                                               category.c_str(), message.c_str())
                       : -1;
        }
        const ZeroDdsPluginLoggerListener* listener;
    } proxy;

    static LoggingAdapter* self(void* user) { return static_cast<LoggingAdapter*>(user); }

    static const char* class_id_of(void* user) { return self(user)->class_id.c_str(); }

    static ZeroDdsPluginStatus set_log_options(void* user, const ZeroDdsPluginLogOptions* o,
                                               ZeroDdsPluginError* err) {
        LoggingAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            LogOptions options;
            options.log_level = o->log_level;
            options.has_log_file = o->log_file != 0;
            options.log_file = text(o->log_file);
            options.distribute = o->distribute;
            a->impl->set_log_options(options);
        });
    }

    static ZeroDdsPluginStatus enable_logging(void* user, ZeroDdsPluginError* err) {
        LoggingAdapter* a = self(user);
        return Guard(&a->message, err).run([&] { a->impl->enable_logging(); });
    }

    static ZeroDdsPluginStatus set_listener(void* user, const ZeroDdsPluginLoggerListener* l,
                                            ZeroDdsPluginError* err) {
        LoggingAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            a->proxy.listener = l;
            a->impl->set_listener(l ? &a->proxy : 0);
        });
    }

    static void log(void* user, std::int32_t level, const std::uint8_t* participant,
                    const char* category, const char* message) {
        LoggingAdapter* a = self(user);
        try {
            a->impl->log(level, guid_of(participant), text(category), text(message));
        } catch (...) {
            // Logging does not fail the operation that logs (header, "Logging").
        }
    }

    static void destroy(void* user) { delete self(user); }
};

class DataTaggingAdapter {
public:
    explicit DataTaggingAdapter(std::unique_ptr<DataTaggingPlugin> p)
        : impl(std::move(p)), class_id(impl->plugin_class_id()) {}

    std::unique_ptr<DataTaggingPlugin> impl;
    std::string class_id;
    std::vector<DataTag> tags;
    std::vector<ZeroDdsPluginDataTag> raw;

    static DataTaggingAdapter* self(void* user) { return static_cast<DataTaggingAdapter*>(user); }

    static const char* class_id_of(void* user) { return self(user)->class_id.c_str(); }

    static void set_tags(void* user, const std::uint8_t* guid, const ZeroDdsPluginDataTag* t,
                         std::size_t n) {
        DataTaggingAdapter* a = self(user);
        std::vector<DataTag> tags;
        for (std::size_t i = 0; i < n; ++i) {
            tags.push_back(DataTag(text(t[i].name), text(t[i].value)));
        }
        try {
            a->impl->set_tags(guid_of(guid), tags);
        } catch (...) {
        }
    }

    static void get_tags(void* user, const std::uint8_t* guid, const ZeroDdsPluginDataTag** out,
                         std::size_t* n) {
        DataTaggingAdapter* a = self(user);
        a->tags.clear();
        a->raw.clear();
        try {
            a->tags = a->impl->get_tags(guid_of(guid));
        } catch (...) {
        }
        for (std::size_t i = 0; i < a->tags.size(); ++i) {
            ZeroDdsPluginDataTag t;
            t.name = a->tags[i].name.c_str();
            t.value = a->tags[i].value.c_str();
            a->raw.push_back(t);
        }
        *out = a->raw.empty() ? 0 : a->raw.data();
        *n = a->raw.size();
    }

    static void destroy(void* user) { delete self(user); }
};

}  // namespace detail

// As set_access_control_plugin, for the Logging plugin.
inline int set_logging_plugin(zerodds_ZeroDdsSecurityConfig* cfg,
                              std::unique_ptr<LoggingPlugin> plugin) {
    using detail::LoggingAdapter;
    LoggingAdapter* adapter = new LoggingAdapter(std::move(plugin));
    ZeroDdsPluginLogging table;
    std::memset(&table, 0, sizeof(table));
    table.abi_version = ZERODDS_PLUGIN_ABI_VERSION;
    table.user = adapter;
    table.plugin_class_id = LoggingAdapter::class_id_of;
    table.set_log_options = LoggingAdapter::set_log_options;
    table.enable_logging = LoggingAdapter::enable_logging;
    table.set_listener = LoggingAdapter::set_listener;
    table.log = LoggingAdapter::log;
    table.destroy = LoggingAdapter::destroy;
    int status = zerodds_security_set_logging_plugin(cfg, &table);
    if (status != 0) {
        delete adapter;
    }
    return status;
}

// As set_access_control_plugin, for the DataTagging plugin.
inline int set_data_tagging_plugin(zerodds_ZeroDdsSecurityConfig* cfg,
                                   std::unique_ptr<DataTaggingPlugin> plugin) {
    using detail::DataTaggingAdapter;
    DataTaggingAdapter* adapter = new DataTaggingAdapter(std::move(plugin));
    ZeroDdsPluginDataTagging table;
    std::memset(&table, 0, sizeof(table));
    table.abi_version = ZERODDS_PLUGIN_ABI_VERSION;
    table.user = adapter;
    table.plugin_class_id = DataTaggingAdapter::class_id_of;
    table.set_tags = DataTaggingAdapter::set_tags;
    table.get_tags = DataTaggingAdapter::get_tags;
    table.destroy = DataTaggingAdapter::destroy;
    int status = zerodds_security_set_data_tagging_plugin(cfg, &table);
    if (status != 0) {
        delete adapter;
    }
    return status;
}


namespace detail {

inline Bytes bytes_view(const ZeroDdsPluginBytes* b) {
    return b ? bytes_of(b->data, b->len) : Bytes();
}

inline ZeroDdsPluginBytes bytes_ref(const Bytes& b) {
    ZeroDdsPluginBytes out;
    out.data = b.empty() ? 0 : b.data();
    out.len = b.size();
    return out;
}

inline AlgorithmMasks algorithm_masks_of(const ZeroDdsPluginAlgorithmMasks& m) {
    AlgorithmMasks out;
    out.trust_chain = masks_of(m.trust_chain);
    out.message_auth = masks_of(m.message_auth);
    out.key_establishment = masks_of(m.key_establishment);
    return out;
}

inline ZeroDdsPluginAlgorithmMasks algorithm_masks_to(const AlgorithmMasks& m) {
    ZeroDdsPluginAlgorithmMasks out;
    out.trust_chain = masks_to(m.trust_chain);
    out.message_auth = masks_to(m.message_auth);
    out.key_establishment = masks_to(m.key_establishment);
    return out;
}

inline void step_to(const HandshakeStep& step, Bytes* keep, ZeroDdsPluginHandshakeStep* out) {
    *keep = step.token;
    out->kind = step.kind;
    out->token = bytes_ref(*keep);
    out->secret = step.secret;
}

class AuthenticationAdapter {
public:
    explicit AuthenticationAdapter(std::unique_ptr<AuthenticationPlugin> p)
        : impl(std::move(p)), class_id(impl->plugin_class_id()) {}

    std::unique_ptr<AuthenticationPlugin> impl;
    std::string class_id;
    std::string message;
    Bytes scratch;
    Bytes scratch2;
    std::vector<std::unique_ptr<Bytes> > identity_tokens;
    std::vector<std::unique_ptr<Bytes> > status_tokens;
    std::vector<std::unique_ptr<Bytes> > credential_tokens;

    struct ListenerProxy : public AuthenticationListener {
        ListenerProxy() : listener(0) {}
        int on_revoke_identity(std::uint64_t handle) {
            return listener && listener->on_revoke_identity
                       ? listener->on_revoke_identity(listener->context, handle)
                       : 1;
        }
        void on_status_changed(std::uint64_t handle, int kind) {
            if (listener && listener->on_status_changed) {
                listener->on_status_changed(listener->context, handle, kind);
            }
        }
        const ZeroDdsPluginAuthenticationListener* listener;
    } proxy;

    static AuthenticationAdapter* self(void* user) {
        return static_cast<AuthenticationAdapter*>(user);
    }

    static const char* class_id_of(void* user) { return self(user)->class_id.c_str(); }

    static ZeroDdsPluginStatus validate_local(void* user, std::uint32_t domain,
                                              const ZeroDdsPluginProperty* props, std::size_t n,
                                              const ZeroDdsPluginBinaryProperty* bins,
                                              std::size_t nb, const std::uint8_t* guid,
                                              std::int32_t* out_result, std::uint64_t* out_identity,
                                              std::uint8_t* out_guid, ZeroDdsPluginError* err) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            std::vector<BinaryProperty> binary;
            for (std::size_t i = 0; i < nb; ++i) {
                BinaryProperty b;
                b.name = text(bins[i].name);
                b.value = bytes_of(bins[i].value, bins[i].value_len);
                b.propagate = bins[i].propagate;
                binary.push_back(b);
            }
            LocalIdentityValidation v =
                a->impl->validate_local_identity(domain, properties_of(props, n), binary, guid_of(guid));
            *out_result = v.result;
            *out_identity = v.identity;
            std::memcpy(out_guid, v.adjusted_guid.data(), 16);
        });
    }

    static ZeroDdsPluginStatus token(void* user, std::uint64_t identity, ZeroDdsPluginBytes* out,
                                     ZeroDdsPluginError* err, bool status) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            std::unique_ptr<Bytes> issued(new Bytes(status ? a->impl->get_identity_status_token(identity)
                                                           : a->impl->get_identity_token(identity)));
            *out = bytes_ref(*issued);
            (status ? a->status_tokens : a->identity_tokens).push_back(std::move(issued));
        });
    }

    static ZeroDdsPluginStatus identity_token(void* u, std::uint64_t i, ZeroDdsPluginBytes* o,
                                              ZeroDdsPluginError* e) {
        return token(u, i, o, e, false);
    }
    static ZeroDdsPluginStatus status_token(void* u, std::uint64_t i, ZeroDdsPluginBytes* o,
                                            ZeroDdsPluginError* e) {
        return token(u, i, o, e, true);
    }

    static ZeroDdsPluginStatus set_config(void* user, std::uint64_t identity,
                                          const ZeroDdsPluginAlgorithmMasks* config,
                                          ZeroDdsPluginAlgorithmMasks* out, ZeroDdsPluginError* err) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = algorithm_masks_to(
                a->impl->set_participant_security_config(identity, algorithm_masks_of(*config)));
        });
    }

    static ZeroDdsPluginStatus set_credential(void* user, std::uint64_t identity,
                                              const ZeroDdsPluginBytes* credential,
                                              const ZeroDdsPluginBytes* permissions,
                                              ZeroDdsPluginError* err) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            a->impl->set_permissions_credential_and_token(identity, bytes_view(credential),
                                                          bytes_view(permissions));
        });
    }

    static ZeroDdsPluginStatus validate_remote(void* user, std::uint64_t local,
                                               const std::uint8_t* guid,
                                               const ZeroDdsPluginBytes* token,
                                               const ZeroDdsPluginBytes* request,
                                               std::int32_t* out_result, std::uint64_t* out_identity,
                                               ZeroDdsPluginBytes* out_request,
                                               ZeroDdsPluginError* err) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            Bytes request_copy = bytes_view(request);
            RemoteIdentityValidation v = a->impl->validate_remote_identity(
                local, guid_of(guid), bytes_view(token), request ? &request_copy : 0);
            *out_result = v.result;
            *out_identity = v.identity;
            a->scratch = v.local_auth_request;
            out_request->data = v.has_local_auth_request ? a->scratch.data() : 0;
            out_request->len = v.has_local_auth_request ? a->scratch.size() : 0;
        });
    }

    static ZeroDdsPluginStatus begin_request(void* user, std::uint64_t initiator,
                                             std::uint64_t replier, const ZeroDdsPluginBytes* data,
                                             std::uint64_t* out_handshake,
                                             ZeroDdsPluginHandshakeStep* out_step,
                                             ZeroDdsPluginError* err) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            step_to(a->impl->begin_handshake_request(initiator, replier, bytes_view(data),
                                                     out_handshake),
                    &a->scratch, out_step);
        });
    }

    static ZeroDdsPluginStatus begin_reply(void* user, std::uint64_t replier, std::uint64_t initiator,
                                           const ZeroDdsPluginBytes* request,
                                           const ZeroDdsPluginBytes* data,
                                           std::uint64_t* out_handshake,
                                           ZeroDdsPluginHandshakeStep* out_step,
                                           ZeroDdsPluginError* err) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            step_to(a->impl->begin_handshake_reply(replier, initiator, bytes_view(request),
                                                   bytes_view(data), out_handshake),
                    &a->scratch, out_step);
        });
    }

    static ZeroDdsPluginStatus process(void* user, std::uint64_t handshake,
                                       const ZeroDdsPluginBytes* token,
                                       ZeroDdsPluginHandshakeStep* out_step,
                                       ZeroDdsPluginError* err) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            step_to(a->impl->process_handshake(handshake, bytes_view(token)), &a->scratch, out_step);
        });
    }

    static ZeroDdsPluginStatus shared_secret(void* user, std::uint64_t handshake,
                                             std::uint64_t* out, ZeroDdsPluginError* err) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] { *out = a->impl->get_shared_secret(handshake); });
    }

    static ZeroDdsPluginStatus peer_credential(void* user, std::uint64_t handshake,
                                               ZeroDdsPluginBytes* out, ZeroDdsPluginError* err) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            std::unique_ptr<Bytes> issued(
                new Bytes(a->impl->get_authenticated_peer_credential_token(handshake)));
            *out = bytes_ref(*issued);
            a->credential_tokens.push_back(std::move(issued));
        });
    }

    static ZeroDdsPluginStatus set_listener_c(void* user,
                                              const ZeroDdsPluginAuthenticationListener* l,
                                              ZeroDdsPluginError* err) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            a->proxy.listener = l;
            a->impl->set_listener(l ? &a->proxy : 0);
        });
    }

    static ZeroDdsPluginStatus give_back(void* user, const ZeroDdsPluginBytes* t,
                                         ZeroDdsPluginError* err, int which) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            std::vector<std::unique_ptr<Bytes> >& list =
                which == 0 ? a->identity_tokens : which == 1 ? a->status_tokens : a->credential_tokens;
            std::unique_ptr<Bytes> found;
            for (std::vector<std::unique_ptr<Bytes> >::iterator it = list.begin(); it != list.end();
                 ++it) {
                if (t->data == ((*it)->empty() ? 0 : (*it)->data())) {
                    found = std::move(*it);
                    list.erase(it);
                    break;
                }
            }
            if (!found) {
                throw SecurityException(kBadArgument, "the token was not issued by this plugin");
            }
            if (which == 0) {
                a->impl->return_identity_token(*found);
            } else if (which == 1) {
                a->impl->return_identity_status_token(*found);
            } else {
                a->impl->return_authenticated_peer_credential_token(*found);
            }
        });
    }

    static ZeroDdsPluginStatus return_identity_token_c(void* u, const ZeroDdsPluginBytes* t,
                                                       ZeroDdsPluginError* e) {
        return give_back(u, t, e, 0);
    }
    static ZeroDdsPluginStatus return_status_token_c(void* u, const ZeroDdsPluginBytes* t,
                                                     ZeroDdsPluginError* e) {
        return give_back(u, t, e, 1);
    }
    static ZeroDdsPluginStatus return_credential_token_c(void* u, const ZeroDdsPluginBytes* t,
                                                         ZeroDdsPluginError* e) {
        return give_back(u, t, e, 2);
    }

    static ZeroDdsPluginStatus return_handle(void* user, std::uint64_t handle,
                                             ZeroDdsPluginError* err, int which) {
        AuthenticationAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            if (which == 0) {
                a->impl->return_handshake_handle(handle);
            } else if (which == 1) {
                a->impl->return_identity_handle(handle);
            } else {
                a->impl->return_sharedsecret_handle(handle);
            }
        });
    }
    static ZeroDdsPluginStatus return_handshake_c(void* u, std::uint64_t h, ZeroDdsPluginError* e) {
        return return_handle(u, h, e, 0);
    }
    static ZeroDdsPluginStatus return_identity_c(void* u, std::uint64_t h, ZeroDdsPluginError* e) {
        return return_handle(u, h, e, 1);
    }
    static ZeroDdsPluginStatus return_secret_c(void* u, std::uint64_t h, ZeroDdsPluginError* e) {
        return return_handle(u, h, e, 2);
    }

    static void permissions_token(void* user, ZeroDdsPluginBytes* out) {
        AuthenticationAdapter* a = self(user);
        try {
            a->scratch2 = a->impl->get_permissions_token();
        } catch (...) {
            a->scratch2.clear();
        }
        *out = bytes_ref(a->scratch2);
    }

    static bool secret_bytes(void* user, std::uint64_t secret, ZeroDdsPluginBytes* out) {
        AuthenticationAdapter* a = self(user);
        try {
            if (!a->impl->get_shared_secret_bytes(secret, &a->scratch2)) return false;
        } catch (...) {
            return false;
        }
        *out = bytes_ref(a->scratch2);
        return true;
    }

    static bool secret_challenges(void* user, std::uint64_t secret, std::uint8_t* c1,
                                  std::uint8_t* c2) {
        try {
            return self(user)->impl->get_shared_secret_challenges(secret, c1, c2);
        } catch (...) {
            return false;
        }
    }

    static void destroy(void* user) { delete self(user); }
};

}  // namespace detail

// As set_access_control_plugin, for the Authentication plugin.
inline int set_authentication_plugin(zerodds_ZeroDdsSecurityConfig* cfg,
                                     std::unique_ptr<AuthenticationPlugin> plugin) {
    using detail::AuthenticationAdapter;
    AuthenticationAdapter* adapter = new AuthenticationAdapter(std::move(plugin));
    ZeroDdsPluginAuthentication table;
    std::memset(&table, 0, sizeof(table));
    table.abi_version = ZERODDS_PLUGIN_ABI_VERSION;
    table.user = adapter;
    table.plugin_class_id = AuthenticationAdapter::class_id_of;
    table.validate_local_identity = AuthenticationAdapter::validate_local;
    table.get_identity_token = AuthenticationAdapter::identity_token;
    table.get_identity_status_token = AuthenticationAdapter::status_token;
    table.set_participant_security_config = AuthenticationAdapter::set_config;
    table.set_permissions_credential_and_token = AuthenticationAdapter::set_credential;
    table.validate_remote_identity = AuthenticationAdapter::validate_remote;
    table.begin_handshake_request = AuthenticationAdapter::begin_request;
    table.begin_handshake_reply = AuthenticationAdapter::begin_reply;
    table.process_handshake = AuthenticationAdapter::process;
    table.get_shared_secret = AuthenticationAdapter::shared_secret;
    table.get_authenticated_peer_credential_token = AuthenticationAdapter::peer_credential;
    table.set_listener = AuthenticationAdapter::set_listener_c;
    table.return_identity_token = AuthenticationAdapter::return_identity_token_c;
    table.return_identity_status_token = AuthenticationAdapter::return_status_token_c;
    table.return_authenticated_peer_credential_token = AuthenticationAdapter::return_credential_token_c;
    table.return_handshake_handle = AuthenticationAdapter::return_handshake_c;
    table.return_identity_handle = AuthenticationAdapter::return_identity_c;
    table.return_sharedsecret_handle = AuthenticationAdapter::return_secret_c;
    table.get_permissions_token = AuthenticationAdapter::permissions_token;
    table.get_shared_secret_bytes = AuthenticationAdapter::secret_bytes;
    table.get_shared_secret_challenges = AuthenticationAdapter::secret_challenges;
    table.destroy = AuthenticationAdapter::destroy;
    int status = zerodds_security_set_authentication_plugin(cfg, &table);
    if (status != 0) {
        delete adapter;
    }
    return status;
}

namespace detail {

inline std::vector<DataHolder> holders_of(const ZeroDdsPluginDataHolder* h, std::size_t n) {
    std::vector<DataHolder> out;
    for (std::size_t i = 0; i < n; ++i) out.push_back(holder_of(h[i]));
    return out;
}

// A list of tokens handed out and not yet given back.
struct IssuedTokens {
    std::vector<DataHolder> value;
    std::vector<std::unique_ptr<IssuedHolder> > holders;
    std::vector<ZeroDdsPluginDataHolder> raw;

    explicit IssuedTokens(const std::vector<DataHolder>& v) : value(v) {
        for (std::size_t i = 0; i < value.size(); ++i) {
            holders.push_back(std::unique_ptr<IssuedHolder>(new IssuedHolder(value[i])));
            raw.push_back(holders.back()->raw);
        }
    }

private:
    IssuedTokens(const IssuedTokens&);
    IssuedTokens& operator=(const IssuedTokens&);
};

inline bool same_tokens(const std::vector<DataHolder>& a, const std::vector<DataHolder>& b) {
    if (a.size() != b.size()) return false;
    for (std::size_t i = 0; i < a.size(); ++i) {
        if (a[i].class_id != b[i].class_id || !same(a[i].properties, b[i].properties)) return false;
    }
    return true;
}

class CryptographicAdapter {
public:
    explicit CryptographicAdapter(std::unique_ptr<CryptographicPlugin> p) : impl(std::move(p)) {}

    std::unique_ptr<CryptographicPlugin> impl;
    std::string message;
    Bytes scratch;
    Bytes scratch2;
    std::vector<std::unique_ptr<IssuedTokens> > issued;

    static CryptographicAdapter* self(void* user) { return static_cast<CryptographicAdapter*>(user); }

    static std::vector<std::uint64_t> handles(const std::uint64_t* h, std::size_t n) {
        return n ? std::vector<std::uint64_t>(h, h + n) : std::vector<std::uint64_t>();
    }

    static ZeroDdsPluginStatus register_participant(
        void* user, std::uint64_t identity, std::uint64_t permissions,
        const ZeroDdsPluginProperty* props, std::size_t n,
        const ZeroDdsPluginParticipantConfig* config, std::uint64_t* out,
        ZeroDdsPluginParticipantAlgorithmInfo* info, ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            ParticipantAlgorithmInfo result;
            *out = a->impl->register_local_participant(identity, permissions, properties_of(props, n),
                                                       participant_config_of(*config), &result);
            ParticipantConfig carrier;
            carrier.algorithm_info = result;
            IssuedParticipantConfig converted(carrier);
            *info = converted.raw.algorithm_info;
        });
    }

    static ZeroDdsPluginStatus register_remote_participant(void* user, std::uint64_t local,
                                                           std::uint64_t identity,
                                                           std::uint64_t permissions,
                                                           std::uint64_t secret, std::uint64_t* out,
                                                           ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run(
            [&] { *out = a->impl->register_matched_remote_participant(local, identity, permissions, secret); });
    }

    static ZeroDdsPluginStatus register_datawriter(
        void* user, std::uint64_t participant, const ZeroDdsPluginProperty* props, std::size_t n,
        const ZeroDdsPluginEndpointConfig* config, const std::uint8_t* guid, std::uint64_t* out,
        ZeroDdsPluginEndpointAlgorithmInfo* info, ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            EndpointAlgorithmInfo result;
            *out = a->impl->register_local_datawriter(participant, properties_of(props, n),
                                                      endpoint_config_of(*config), guid_of(guid),
                                                      &result);
            info->required_mask = result.required_mask;
            info->supported_mask = result.supported_mask;
        });
    }

    static ZeroDdsPluginStatus register_datareader(
        void* user, std::uint64_t participant, const ZeroDdsPluginProperty* props, std::size_t n,
        const ZeroDdsPluginEndpointConfig* config, const std::uint8_t* guid, std::uint64_t* out,
        ZeroDdsPluginEndpointAlgorithmInfo* info, ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            EndpointAlgorithmInfo result;
            *out = a->impl->register_local_datareader(participant, properties_of(props, n),
                                                      endpoint_config_of(*config), guid_of(guid),
                                                      &result);
            info->required_mask = result.required_mask;
            info->supported_mask = result.supported_mask;
        });
    }

    static ZeroDdsPluginStatus register_remote_datareader(void* user, std::uint64_t writer,
                                                          std::uint64_t participant,
                                                          std::uint64_t secret, bool relay_only,
                                                          std::uint64_t* out, ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = a->impl->register_matched_remote_datareader(writer, participant, secret, relay_only);
        });
    }

    static ZeroDdsPluginStatus register_remote_datawriter(void* user, std::uint64_t reader,
                                                          std::uint64_t participant,
                                                          std::uint64_t secret, std::uint64_t* out,
                                                          ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *out = a->impl->register_matched_remote_datawriter(reader, participant, secret);
        });
    }

    static ZeroDdsPluginStatus revise(void* user, std::uint64_t participant, std::int32_t* out,
                                      ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] { *out = a->impl->revise_local_entity_keys(participant); });
    }

    static ZeroDdsPluginStatus activate(void* user, std::uint64_t participant, std::int32_t revision,
                                        bool* out, ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run(
            [&] { *out = a->impl->activate_key_revision(participant, revision); });
    }

    static ZeroDdsPluginStatus unregister_participant(void* user, std::uint64_t h, bool* out,
                                                      ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] { *out = a->impl->unregister_participant(h); });
    }
    static ZeroDdsPluginStatus unregister_datawriter(void* user, std::uint64_t h, bool* out,
                                                     ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] { *out = a->impl->unregister_datawriter(h); });
    }
    static ZeroDdsPluginStatus unregister_datareader(void* user, std::uint64_t h, bool* out,
                                                     ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] { *out = a->impl->unregister_datareader(h); });
    }

    static ZeroDdsPluginStatus create_tokens(void* user, std::uint64_t first, std::uint64_t second,
                                             std::int32_t revision, ZeroDdsPluginCryptoTokens* out,
                                             ZeroDdsPluginError* err, int which) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            std::vector<DataHolder> tokens;
            bool present = which == 0
                               ? a->impl->create_local_participant_crypto_tokens(first, second, revision, &tokens)
                               : which == 1
                                     ? a->impl->create_local_datawriter_crypto_tokens(first, second, revision, &tokens)
                                     : a->impl->create_local_datareader_crypto_tokens(first, second, revision, &tokens);
            out->present = present;
            out->tokens = 0;
            out->n = 0;
            if (!present) return;
            std::unique_ptr<IssuedTokens> list(new IssuedTokens(tokens));
            out->tokens = list->raw.empty() ? 0 : list->raw.data();
            out->n = list->raw.size();
            a->issued.push_back(std::move(list));
        });
    }
    static ZeroDdsPluginStatus create_participant_tokens(void* u, std::uint64_t f, std::uint64_t s,
                                                         std::int32_t r, ZeroDdsPluginCryptoTokens* o,
                                                         ZeroDdsPluginError* e) {
        return create_tokens(u, f, s, r, o, e, 0);
    }
    static ZeroDdsPluginStatus create_datawriter_tokens(void* u, std::uint64_t f, std::uint64_t s,
                                                        std::int32_t r, ZeroDdsPluginCryptoTokens* o,
                                                        ZeroDdsPluginError* e) {
        return create_tokens(u, f, s, r, o, e, 1);
    }
    static ZeroDdsPluginStatus create_datareader_tokens(void* u, std::uint64_t f, std::uint64_t s,
                                                        std::int32_t r, ZeroDdsPluginCryptoTokens* o,
                                                        ZeroDdsPluginError* e) {
        return create_tokens(u, f, s, r, o, e, 2);
    }

    static ZeroDdsPluginStatus set_tokens(void* user, std::uint64_t first, std::uint64_t second,
                                          const ZeroDdsPluginDataHolder* tokens, std::size_t n,
                                          bool* out, ZeroDdsPluginError* err, int which) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            std::vector<DataHolder> list = holders_of(tokens, n);
            *out = which == 0 ? a->impl->set_remote_participant_crypto_tokens(first, second, list)
                              : which == 1 ? a->impl->set_remote_datawriter_crypto_tokens(first, second, list)
                                           : a->impl->set_remote_datareader_crypto_tokens(first, second, list);
        });
    }
    static ZeroDdsPluginStatus set_participant_tokens(void* u, std::uint64_t f, std::uint64_t s,
                                                      const ZeroDdsPluginDataHolder* t, std::size_t n,
                                                      bool* o, ZeroDdsPluginError* e) {
        return set_tokens(u, f, s, t, n, o, e, 0);
    }
    static ZeroDdsPluginStatus set_datawriter_tokens(void* u, std::uint64_t f, std::uint64_t s,
                                                     const ZeroDdsPluginDataHolder* t, std::size_t n,
                                                     bool* o, ZeroDdsPluginError* e) {
        return set_tokens(u, f, s, t, n, o, e, 1);
    }
    static ZeroDdsPluginStatus set_datareader_tokens(void* u, std::uint64_t f, std::uint64_t s,
                                                     const ZeroDdsPluginDataHolder* t, std::size_t n,
                                                     bool* o, ZeroDdsPluginError* e) {
        return set_tokens(u, f, s, t, n, o, e, 2);
    }

    static ZeroDdsPluginStatus return_tokens(void* user, const ZeroDdsPluginDataHolder* tokens,
                                             std::size_t n, bool* out, ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            std::vector<DataHolder> list = holders_of(tokens, n);
            for (std::vector<std::unique_ptr<IssuedTokens> >::iterator it = a->issued.begin();
                 it != a->issued.end(); ++it) {
                if (same_tokens((*it)->value, list)) {
                    a->issued.erase(it);
                    break;
                }
            }
            *out = a->impl->return_crypto_tokens(list);
        });
    }

    static ZeroDdsPluginStatus encode_payload(void* user, const ZeroDdsPluginBytes* plain,
                                              std::uint64_t writer, ZeroDdsPluginEncodedPayload* out,
                                              ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            EncodedPayload payload;
            out->present = a->impl->encode_serialized_payload(bytes_view(plain), writer, &payload);
            a->scratch = payload.encoded;
            a->scratch2 = payload.extra_inline_qos;
            out->encoded = bytes_ref(a->scratch);
            out->extra_inline_qos = bytes_ref(a->scratch2);
        });
    }

    static ZeroDdsPluginStatus encode_datawriter(void* user, const ZeroDdsPluginBytes* plain,
                                                 std::uint64_t writer, const std::uint64_t* readers,
                                                 std::size_t n, std::size_t* index,
                                                 ZeroDdsPluginBytes* out, bool* ok,
                                                 ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *ok = a->impl->encode_datawriter_submessage(bytes_view(plain), writer, handles(readers, n),
                                                        index, &a->scratch);
            *out = bytes_ref(a->scratch);
        });
    }

    static ZeroDdsPluginStatus encode_datareader(void* user, const ZeroDdsPluginBytes* plain,
                                                 std::uint64_t reader, const std::uint64_t* writers,
                                                 std::size_t n, bool* present,
                                                 ZeroDdsPluginBytes* out, ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *present = a->impl->encode_datareader_submessage(bytes_view(plain), reader,
                                                             handles(writers, n), &a->scratch);
            *out = bytes_ref(a->scratch);
        });
    }

    static ZeroDdsPluginStatus encode_message(void* user, const ZeroDdsPluginBytes* plain,
                                              std::uint64_t participant,
                                              const std::uint64_t* receivers, std::size_t n,
                                              std::size_t* index, bool psk, ZeroDdsPluginBytes* out,
                                              bool* ok, ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *ok = a->impl->encode_rtps_message(bytes_view(plain), participant, handles(receivers, n),
                                               index, psk, &a->scratch);
            *out = bytes_ref(a->scratch);
        });
    }

    static ZeroDdsPluginStatus decode_message(void* user, const ZeroDdsPluginBytes* encoded,
                                              std::uint64_t receiving, std::uint64_t sending,
                                              bool* present, ZeroDdsPluginBytes* out,
                                              ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *present = a->impl->decode_rtps_message(bytes_view(encoded), receiving, sending, &a->scratch);
            *out = bytes_ref(a->scratch);
        });
    }

    static ZeroDdsPluginStatus preprocess(void* user, const ZeroDdsPluginBytes* encoded,
                                          std::uint64_t receiving, std::uint64_t sending,
                                          bool* present, std::uint64_t* writer,
                                          std::uint64_t* reader, std::int32_t* category,
                                          ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            PreprocessedSubmessage result;
            *present = a->impl->preprocess_secure_submsg(bytes_view(encoded), receiving, sending, &result);
            *writer = result.datawriter;
            *reader = result.datareader;
            *category = result.category;
        });
    }

    static ZeroDdsPluginStatus decode_submessage(void* user, const ZeroDdsPluginBytes* encoded,
                                                 std::uint64_t first, std::uint64_t second,
                                                 bool* present, ZeroDdsPluginBytes* out,
                                                 ZeroDdsPluginError* err, bool writer) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *present = writer ? a->impl->decode_datawriter_submessage(bytes_view(encoded), first, second, &a->scratch)
                              : a->impl->decode_datareader_submessage(bytes_view(encoded), first, second, &a->scratch);
            *out = bytes_ref(a->scratch);
        });
    }
    static ZeroDdsPluginStatus decode_datawriter(void* u, const ZeroDdsPluginBytes* e, std::uint64_t f,
                                                 std::uint64_t s, bool* p, ZeroDdsPluginBytes* o,
                                                 ZeroDdsPluginError* r) {
        return decode_submessage(u, e, f, s, p, o, r, true);
    }
    static ZeroDdsPluginStatus decode_datareader(void* u, const ZeroDdsPluginBytes* e, std::uint64_t f,
                                                 std::uint64_t s, bool* p, ZeroDdsPluginBytes* o,
                                                 ZeroDdsPluginError* r) {
        return decode_submessage(u, e, f, s, p, o, r, false);
    }

    static ZeroDdsPluginStatus decode_payload(void* user, const ZeroDdsPluginBytes* encoded,
                                              const ZeroDdsPluginBytes* inline_qos,
                                              std::uint64_t reader, std::uint64_t writer,
                                              bool* present, ZeroDdsPluginBytes* out,
                                              ZeroDdsPluginError* err) {
        CryptographicAdapter* a = self(user);
        return Guard(&a->message, err).run([&] {
            *present = a->impl->decode_serialized_payload(bytes_view(encoded), bytes_view(inline_qos),
                                                          reader, writer, &a->scratch);
            *out = bytes_ref(a->scratch);
        });
    }

    static void destroy(void* user) { delete self(user); }
};

}  // namespace detail

// As set_access_control_plugin, for the Cryptographic plugin. Needs an
// Authentication plugin of the application as well.
inline int set_cryptographic_plugin(zerodds_ZeroDdsSecurityConfig* cfg,
                                    std::unique_ptr<CryptographicPlugin> plugin) {
    using detail::CryptographicAdapter;
    CryptographicAdapter* adapter = new CryptographicAdapter(std::move(plugin));
    ZeroDdsPluginCryptographic table;
    std::memset(&table, 0, sizeof(table));
    table.abi_version = ZERODDS_PLUGIN_ABI_VERSION;
    table.user = adapter;
    table.register_local_participant = CryptographicAdapter::register_participant;
    table.register_matched_remote_participant = CryptographicAdapter::register_remote_participant;
    table.register_local_datawriter = CryptographicAdapter::register_datawriter;
    table.register_matched_remote_datareader = CryptographicAdapter::register_remote_datareader;
    table.register_local_datareader = CryptographicAdapter::register_datareader;
    table.register_matched_remote_datawriter = CryptographicAdapter::register_remote_datawriter;
    table.revise_local_entity_keys = CryptographicAdapter::revise;
    table.activate_key_revision = CryptographicAdapter::activate;
    table.unregister_participant = CryptographicAdapter::unregister_participant;
    table.unregister_datawriter = CryptographicAdapter::unregister_datawriter;
    table.unregister_datareader = CryptographicAdapter::unregister_datareader;
    table.create_local_participant_crypto_tokens = CryptographicAdapter::create_participant_tokens;
    table.set_remote_participant_crypto_tokens = CryptographicAdapter::set_participant_tokens;
    table.create_local_datawriter_crypto_tokens = CryptographicAdapter::create_datawriter_tokens;
    table.set_remote_datawriter_crypto_tokens = CryptographicAdapter::set_datawriter_tokens;
    table.create_local_datareader_crypto_tokens = CryptographicAdapter::create_datareader_tokens;
    table.set_remote_datareader_crypto_tokens = CryptographicAdapter::set_datareader_tokens;
    table.return_crypto_tokens = CryptographicAdapter::return_tokens;
    table.encode_serialized_payload = CryptographicAdapter::encode_payload;
    table.encode_datawriter_submessage = CryptographicAdapter::encode_datawriter;
    table.encode_datareader_submessage = CryptographicAdapter::encode_datareader;
    table.encode_rtps_message = CryptographicAdapter::encode_message;
    table.decode_rtps_message = CryptographicAdapter::decode_message;
    table.preprocess_secure_submsg = CryptographicAdapter::preprocess;
    table.decode_datawriter_submessage = CryptographicAdapter::decode_datawriter;
    table.decode_datareader_submessage = CryptographicAdapter::decode_datareader;
    table.decode_serialized_payload = CryptographicAdapter::decode_payload;
    table.destroy = CryptographicAdapter::destroy;
    int status = zerodds_security_set_cryptographic_plugin(cfg, &table);
    if (status != 0) {
        delete adapter;
    }
    return status;
}

}  // namespace security
}  // namespace zerodds

#endif  // ZERODDS_SECURITY_PLUGINS_HPP
