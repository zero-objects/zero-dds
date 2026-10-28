// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// A DDS-Security 1.2 Authentication, AccessControl, Cryptographic, Logging and
// DataTagging plugin written in C++ and placed into a participant.
//
//     plugin_test <domain> <identity_ca> <identity_cert> <identity_key>
//                 <permissions_ca> <governance.p7s> <permissions.p7s>
//
// The AccessControl plugin refuses the DataWriter and the DataReader of the
// topic "Forbidden" and permits every other operation. The Logging plugin
// counts what the runtime logs; the DataTagging plugin gives every endpoint
// the tag owner=cpp-plugin. The program checks that the runtime called each
// plugin where the specification calls it and that it deleted each plugin
// when it let go of it; it prints "OK" and returns 0, or names the first
// check that failed.

#include <unistd.h>

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>

#include "zerodds_security_plugins.hpp"

using namespace zerodds::security;

namespace {

const char* const kForbidden = "Forbidden";

struct Counters {
    int validate_local, create_participant, create_writer, create_reader;
    int refused_writer, refused_reader;
    int permissions_token_out, permissions_token_back;
    int credential_token_out, credential_token_back;
    int participant_config_out, participant_config_back;
    int writer_config_out, writer_config_back;
    int reader_config_out, reader_config_back;
    int listener_set, ac_destroyed;
    int options, enabled, messages, denied, lg_destroyed;
    int set_tags, get_tags, tg_destroyed;
    int au_validate_local, au_governance_property, au_identity_out, au_identity_back;
    int au_status_out, au_status_back, au_credential_set, au_config_set, au_credential_first;
    int au_destroyed;
    int cr_register_participant, cr_participant_axk, cr_volatile_writer, cr_volatile_reader;
    int cr_protected_writer, cr_destroyed;
};
Counters c;

class Access : public AccessControlPlugin {
public:
    ~Access() { ++c.ac_destroyed; }
    std::string plugin_class_id() const { return "DDS:Access:Cpp"; }

    std::uint64_t validate_local_permissions(std::uint64_t, const ParticipantRequest&) {
        ++c.validate_local;
        return 77;
    }
    std::uint64_t validate_remote_permissions(std::uint64_t, std::uint64_t, const DataHolder&,
                                              const DataHolder&) {
        return 78;
    }
    bool check_create_participant(std::uint64_t permissions, const ParticipantRequest& p) {
        c.create_participant += (permissions == 77 && p.domain_id < 233) ? 1 : 0;
        return true;
    }
    bool check_create_datawriter(std::uint64_t, const EndpointRequest& r) {
        ++c.create_writer;
        bool permit = r.topic_name != kForbidden;
        if (!permit) ++c.refused_writer;
        return permit;
    }
    bool check_create_datareader(std::uint64_t, const EndpointRequest& r) {
        ++c.create_reader;
        bool permit = r.topic_name != kForbidden;
        if (!permit) ++c.refused_reader;
        return permit;
    }
    bool check_create_topic(std::uint64_t, const TopicRequest&) { return true; }
    bool check_local_datawriter_register_instance(std::uint64_t, const Guid&,
                                                  const std::vector<std::uint8_t>&) {
        return true;
    }
    bool check_local_datawriter_dispose_instance(std::uint64_t, const Guid&,
                                                 const std::vector<std::uint8_t>&) {
        return true;
    }
    bool check_remote_participant(std::uint64_t, const ParticipantRequest&) { return true; }
    bool check_remote_datawriter(std::uint64_t, const EndpointRequest&) { return true; }
    RemoteReaderDecision check_remote_datareader(std::uint64_t, const EndpointRequest&) {
        return kPermit;
    }
    bool check_remote_topic(std::uint64_t, const TopicRequest&) { return true; }
    bool check_local_datawriter_match(std::uint64_t, std::uint64_t, const EndpointRequest&,
                                      const EndpointRequest&) {
        return true;
    }
    bool check_local_datareader_match(std::uint64_t, std::uint64_t, const EndpointRequest&,
                                      const EndpointRequest&) {
        return true;
    }
    bool check_remote_datawriter_register_instance(std::uint64_t, const Guid&, const Guid&,
                                                   const std::vector<std::uint8_t>&) {
        return true;
    }
    bool check_remote_datawriter_dispose_instance(std::uint64_t, const Guid&, const Guid&,
                                                  const std::vector<std::uint8_t>&) {
        return true;
    }
    bool check_relay(std::uint64_t, RelayRole, const EndpointRequest&) { return false; }

    DataHolder get_permissions_token(std::uint64_t) {
        ++c.permissions_token_out;
        DataHolder h;
        h.class_id = "DDS:Access:Permissions:Cpp";
        h.properties.push_back(Property("owner", "cpp-plugin"));
        return h;
    }
    DataHolder get_permissions_credential_token(std::uint64_t) {
        ++c.credential_token_out;
        DataHolder h;
        h.class_id = "DDS:Access:Credential:Cpp";
        h.properties.push_back(Property("owner", "cpp-plugin"));
        return h;
    }
    void set_listener(AccessControlListener*) { ++c.listener_set; }
    void return_permissions_token(const DataHolder& t) {
        if (t.class_id != "DDS:Access:Permissions:Cpp") {
            throw SecurityException(kBadArgument, "not the token this plugin issued");
        }
        ++c.permissions_token_back;
    }
    void return_permissions_credential_token(const DataHolder& t) {
        if (t.class_id != "DDS:Access:Credential:Cpp") {
            throw SecurityException(kBadArgument, "not the token this plugin issued");
        }
        ++c.credential_token_back;
    }

    ParticipantConfig get_participant_security_config(std::uint64_t) {
        ++c.participant_config_out;
        // The plugin allows every algorithm and requires none.
        ParticipantConfig config;
        ParticipantAlgorithmInfo& a = config.algorithm_info;
        a.trust_chain.supported_mask = 0xffffffffu;
        a.message_auth.supported_mask = 0xffffffffu;
        a.shared_secret.supported_mask = 0xffffffffu;
        a.cipher_supported_mask = 0xffffffffu;
        return config;
    }
    TopicConfig get_topic_security_config(std::uint64_t, const std::string&) {
        return TopicConfig();
    }
    EndpointConfig get_datawriter_security_config(std::uint64_t, const EndpointRequest& r) {
        ++c.writer_config_out;
        EndpointConfig config;
        // The payload of the topic "Protected" is protected, and encrypted.
        if (r.topic_name == "Protected") {
            config.is_payload_protected = true;
            config.plugin_endpoint_attributes = 2;
        }
        return config;
    }
    EndpointConfig get_datareader_security_config(std::uint64_t, const EndpointRequest&) {
        ++c.reader_config_out;
        return EndpointConfig();
    }
    void return_participant_security_config(const ParticipantConfig&) {
        ++c.participant_config_back;
    }
    void return_topic_security_config(const TopicConfig&) {}
    void return_datawriter_security_config(const EndpointConfig&) { ++c.writer_config_back; }
    void return_datareader_security_config(const EndpointConfig&) { ++c.reader_config_back; }
};

const char kIdentityToken[] = "CPP-IDENTITY-TOKEN";
const char kStatusToken[] = "CPP-IDENTITY-STATUS-TOKEN";

Bytes bytes_of_text(const char* text) {
    return Bytes(text, text + std::strlen(text));
}

class Auth : public AuthenticationPlugin {
public:
    ~Auth() { ++c.au_destroyed; }
    std::string plugin_class_id() const { return "DDS:Auth:Cpp"; }

    LocalIdentityValidation validate_local_identity(std::uint32_t, const std::vector<Property>& props,
                                                    const std::vector<BinaryProperty>&,
                                                    const Guid& guid) {
        ++c.au_validate_local;
        for (std::size_t i = 0; i < props.size(); ++i) {
            if (props[i].name == "dds.sec.access.governance") c.au_governance_property = 1;
        }
        LocalIdentityValidation v;
        v.result = kValidationOk;
        v.identity = 100;
        v.adjusted_guid = guid;
        return v;
    }
    Bytes get_identity_token(std::uint64_t) {
        ++c.au_identity_out;
        return bytes_of_text(kIdentityToken);
    }
    Bytes get_identity_status_token(std::uint64_t) {
        ++c.au_status_out;
        return bytes_of_text(kStatusToken);
    }
    AlgorithmMasks set_participant_security_config(std::uint64_t, const AlgorithmMasks& config) {
        ++c.au_config_set;
        if (c.au_credential_set > 0) c.au_credential_first = 1;
        return config;
    }
    void set_permissions_credential_and_token(std::uint64_t, const Bytes&, const Bytes&) {
        ++c.au_credential_set;
    }
    RemoteIdentityValidation validate_remote_identity(std::uint64_t, const Guid&, const Bytes&,
                                                      const Bytes*) {
        RemoteIdentityValidation v;
        v.result = kValidationFailed;
        return v;
    }
    HandshakeStep begin_handshake_request(std::uint64_t, std::uint64_t, const Bytes&, std::uint64_t*) {
        throw SecurityException(kNotImplemented, "the test has no peer");
    }
    HandshakeStep begin_handshake_reply(std::uint64_t, std::uint64_t, const Bytes&, const Bytes&,
                                        std::uint64_t*) {
        throw SecurityException(kNotImplemented, "the test has no peer");
    }
    HandshakeStep process_handshake(std::uint64_t, const Bytes&) {
        throw SecurityException(kNotImplemented, "the test has no peer");
    }
    std::uint64_t get_shared_secret(std::uint64_t) {
        throw SecurityException(kNotImplemented, "the test has no peer");
    }
    Bytes get_authenticated_peer_credential_token(std::uint64_t) {
        throw SecurityException(kNotImplemented, "the test has no peer");
    }
    void set_listener(AuthenticationListener*) {}
    void return_identity_token(const Bytes& t) {
        if (t != bytes_of_text(kIdentityToken)) throw SecurityException(kBadArgument, "not issued");
        ++c.au_identity_back;
    }
    void return_identity_status_token(const Bytes& t) {
        if (t != bytes_of_text(kStatusToken)) throw SecurityException(kBadArgument, "not issued");
        ++c.au_status_back;
    }
    void return_authenticated_peer_credential_token(const Bytes&) {}
    void return_handshake_handle(std::uint64_t) {}
    void return_identity_handle(std::uint64_t) {}
    void return_sharedsecret_handle(std::uint64_t) {}
    Bytes get_permissions_token() { return bytes_of_text("CPP-PERMISSIONS-TOKEN"); }
};

// The transform of this plugin puts one octet 0xC0 before what it protects.
Bytes wrap(const Bytes& plain) {
    Bytes out(1, 0xC0);
    out.insert(out.end(), plain.begin(), plain.end());
    return out;
}
Bytes unwrap(const Bytes& encoded) {
    return encoded.empty() ? Bytes() : Bytes(encoded.begin() + 1, encoded.end());
}

class Crypto : public CryptographicPlugin {
public:
    ~Crypto() { ++c.cr_destroyed; }

    std::uint64_t register_local_participant(std::uint64_t, std::uint64_t,
                                             const std::vector<Property>&,
                                             const ParticipantConfig& config,
                                             ParticipantAlgorithmInfo*) {
        ++c.cr_register_participant;
        c.cr_participant_axk = config.is_rtps_axk_protected ? 1 : 0;
        return 500;
    }
    std::uint64_t register_matched_remote_participant(std::uint64_t, std::uint64_t, std::uint64_t,
                                                      std::uint64_t) {
        return 501;
    }
    static void note(const std::vector<Property>& props, bool writer) {
        for (std::size_t i = 0; i < props.size(); ++i) {
            if (props[i].name != "dds.sec.builtin_endpoint_name") continue;
            if (writer && props[i].value == "BuiltinParticipantVolatileMessageSecureWriter")
                c.cr_volatile_writer = 1;
            if (!writer && props[i].value == "BuiltinParticipantVolatileMessageSecureReader")
                c.cr_volatile_reader = 1;
        }
    }
    std::uint64_t register_local_datawriter(std::uint64_t, const std::vector<Property>& props,
                                            const EndpointConfig& config, const Guid&,
                                            EndpointAlgorithmInfo*) {
        note(props, true);
        if (config.is_payload_protected) c.cr_protected_writer = 1;
        return 600;
    }
    std::uint64_t register_matched_remote_datareader(std::uint64_t, std::uint64_t, std::uint64_t,
                                                     bool) {
        return 800;
    }
    std::uint64_t register_local_datareader(std::uint64_t, const std::vector<Property>& props,
                                            const EndpointConfig&, const Guid&,
                                            EndpointAlgorithmInfo*) {
        note(props, false);
        return 700;
    }
    std::uint64_t register_matched_remote_datawriter(std::uint64_t, std::uint64_t, std::uint64_t) {
        return 801;
    }
    std::int32_t revise_local_entity_keys(std::uint64_t) { return 1; }
    bool activate_key_revision(std::uint64_t, std::int32_t) { return true; }
    bool unregister_participant(std::uint64_t) { return true; }
    bool unregister_datawriter(std::uint64_t) { return true; }
    bool unregister_datareader(std::uint64_t) { return true; }

    bool create_local_participant_crypto_tokens(std::uint64_t, std::uint64_t, std::int32_t,
                                                std::vector<DataHolder>*) {
        return false;
    }
    bool set_remote_participant_crypto_tokens(std::uint64_t, std::uint64_t,
                                              const std::vector<DataHolder>&) {
        return true;
    }
    bool create_local_datawriter_crypto_tokens(std::uint64_t, std::uint64_t, std::int32_t,
                                               std::vector<DataHolder>*) {
        return false;
    }
    bool set_remote_datawriter_crypto_tokens(std::uint64_t, std::uint64_t,
                                             const std::vector<DataHolder>&) {
        return true;
    }
    bool create_local_datareader_crypto_tokens(std::uint64_t, std::uint64_t, std::int32_t,
                                               std::vector<DataHolder>*) {
        return false;
    }
    bool set_remote_datareader_crypto_tokens(std::uint64_t, std::uint64_t,
                                             const std::vector<DataHolder>&) {
        return true;
    }
    bool return_crypto_tokens(const std::vector<DataHolder>&) { return true; }

    bool encode_serialized_payload(const Bytes& plain, std::uint64_t, EncodedPayload* out) {
        out->encoded = wrap(plain);
        return true;
    }
    bool encode_datawriter_submessage(const Bytes& plain, std::uint64_t,
                                      const std::vector<std::uint64_t>&, std::size_t*, Bytes* out) {
        *out = wrap(plain);
        return true;
    }
    bool encode_datareader_submessage(const Bytes& plain, std::uint64_t,
                                      const std::vector<std::uint64_t>&, Bytes* out) {
        *out = wrap(plain);
        return true;
    }
    bool encode_rtps_message(const Bytes& plain, std::uint64_t, const std::vector<std::uint64_t>&,
                             std::size_t*, bool, Bytes* out) {
        *out = wrap(plain);
        return true;
    }
    bool decode_rtps_message(const Bytes& encoded, std::uint64_t, std::uint64_t, Bytes* out) {
        *out = unwrap(encoded);
        return true;
    }
    bool preprocess_secure_submsg(const Bytes&, std::uint64_t, std::uint64_t,
                                  PreprocessedSubmessage* out) {
        out->category = kSubmessageDatawriter;
        return true;
    }
    bool decode_datawriter_submessage(const Bytes& encoded, std::uint64_t, std::uint64_t,
                                      Bytes* out) {
        *out = unwrap(encoded);
        return true;
    }
    bool decode_datareader_submessage(const Bytes& encoded, std::uint64_t, std::uint64_t,
                                      Bytes* out) {
        *out = unwrap(encoded);
        return true;
    }
    bool decode_serialized_payload(const Bytes& encoded, const Bytes&, std::uint64_t, std::uint64_t,
                                   Bytes* out) {
        *out = unwrap(encoded);
        return true;
    }
};

class Logging : public LoggingPlugin {
public:
    ~Logging() { ++c.lg_destroyed; }
    std::string plugin_class_id() const { return "DDS:Logging:Cpp"; }
    void set_log_options(const LogOptions&) { ++c.options; }
    void enable_logging() { ++c.enabled; }
    void set_listener(LoggerListener*) {}
    void log(int, const Guid&, const std::string& category, const std::string&) {
        ++c.messages;
        if (category == "access.create_datawriter.denied") ++c.denied;
    }
};

class Tagging : public DataTaggingPlugin {
public:
    ~Tagging() { ++c.tg_destroyed; }
    std::string plugin_class_id() const { return "DDS:Tagging:Cpp"; }
    void set_tags(const Guid&, const std::vector<DataTag>&) { ++c.set_tags; }
    std::vector<DataTag> get_tags(const Guid&) {
        ++c.get_tags;
        return std::vector<DataTag>(1, DataTag("owner", "cpp-plugin"));
    }
};

}  // namespace

#define CHECK(cond)                                                              \
    do {                                                                         \
        if (!(cond)) {                                                           \
            std::fprintf(stderr, "check failed: %s (line %d)\n", #cond, __LINE__); \
            return 1;                                                            \
        }                                                                        \
    } while (0)

int main(int argc, char** argv) {
    if (argc != 8) {
        std::fprintf(stderr,
                     "usage: %s domain ca cert key permissions_ca governance permissions\n",
                     argv[0]);
        return 2;
    }
    std::memset(&c, 0, sizeof(c));

    zerodds_ZeroDdsSecurityConfig* cfg = zerodds_security_config_create();
    CHECK(cfg != 0);
    CHECK(zerodds_security_set_identity_ca_path(cfg, argv[2]) == 0);
    CHECK(zerodds_security_set_identity_cert_path(cfg, argv[3]) == 0);
    CHECK(zerodds_security_set_private_key_path(cfg, argv[4]) == 0);
    CHECK(zerodds_security_set_permissions_ca_path(cfg, argv[5]) == 0);
    CHECK(zerodds_security_set_governance_path(cfg, argv[6]) == 0);
    CHECK(zerodds_security_set_permissions_path(cfg, argv[7]) == 0);

    CHECK(set_access_control_plugin(cfg, std::unique_ptr<AccessControlPlugin>(new Access())) == 0);
    CHECK(set_logging_plugin(cfg, std::unique_ptr<LoggingPlugin>(new Logging())) == 0);
    CHECK(set_data_tagging_plugin(cfg, std::unique_ptr<DataTaggingPlugin>(new Tagging())) == 0);
    CHECK(set_authentication_plugin(cfg, std::unique_ptr<AuthenticationPlugin>(new Auth())) == 0);
    CHECK(set_cryptographic_plugin(cfg, std::unique_ptr<CryptographicPlugin>(new Crypto())) == 0);

    zerodds_ZeroDdsRuntime* rt = zerodds_runtime_create_secure(
        static_cast<std::uint32_t>(std::atoi(argv[1])), cfg);
    CHECK(rt != 0);

    // 9.3.2.11: the Authentication plugin validates the identity, with the
    // paths of the configuration as the properties of Table 50; its tokens go
    // back after the copy; the credential reaches it before the config.
    CHECK(c.au_validate_local == 1);
    CHECK(c.au_governance_property == 1);
    CHECK(c.au_identity_out >= 1);
    CHECK(c.au_identity_back == c.au_identity_out);
    CHECK(c.au_status_out >= 1);
    CHECK(c.au_status_back == c.au_status_out);
    CHECK(c.au_credential_set >= 1);
    CHECK(c.au_config_set >= 1);
    CHECK(c.au_credential_first == 1);
    // 9.8.1 step 2 and step 3: validate_local_permissions, then
    // check_create_participant with the handle it gave.
    CHECK(c.validate_local == 1);
    CHECK(c.create_participant == 1);
    // 9.8.1 steps 6 and 7: the tokens, given back after the copy.
    CHECK(c.permissions_token_out >= 1);
    CHECK(c.permissions_token_back == c.permissions_token_out);
    CHECK(c.credential_token_out >= 1);
    CHECK(c.credential_token_back == c.credential_token_out);
    // 9.8.1 step 9: the participant config.
    CHECK(c.participant_config_out >= 1);
    CHECK(c.listener_set >= 1);

    // 9.8.6 steps 5 and 6: the DataWriter.
    zerodds_ZeroDdsWriter* allowed = zerodds_writer_create(rt, "Allowed", "RawBytes", 0);
    CHECK(allowed != 0);
    CHECK(c.create_writer >= 1);
    CHECK(c.writer_config_out >= 1);
    CHECK(c.writer_config_back == c.writer_config_out);
    CHECK(zerodds_writer_create(rt, kForbidden, "RawBytes", 0) == 0);
    CHECK(c.refused_writer >= 1);

    // 9.8.6 steps 9 and 10: the DataReader.
    // 9.8.6 step 6: a topic whose EndpointSecurityConfig protects the payload
    // gets a key from the Cryptographic plugin.
    {
        const std::uint8_t sample[] = {1, 2, 3, 4};
        zerodds_ZeroDdsWriter* protected_writer = zerodds_writer_create(rt, "Protected", "RawBytes", 0);
        CHECK(protected_writer != 0);
        CHECK(c.cr_protected_writer == 1);
        CHECK(zerodds_writer_write(protected_writer, sample, sizeof(sample)) == 0);
        zerodds_writer_destroy(protected_writer);
    }
    // 9.8.1 step 11, 9.8.9.1: the Cryptographic plugin registers the
    // participant, with the config of the AccessControl plugin, and the
    // volatile message endpoints under the names of the specification.
    CHECK(c.cr_register_participant == 1);
    CHECK(c.cr_participant_axk == 0);
    CHECK(c.cr_volatile_writer == 1);
    CHECK(c.cr_volatile_reader == 1);

    zerodds_ZeroDdsReader* reader = zerodds_reader_create(rt, "Allowed", "RawBytes", 0);
    CHECK(reader != 0);
    CHECK(c.create_reader >= 1);
    CHECK(c.reader_config_out >= 1);
    CHECK(c.reader_config_back == c.reader_config_out);
    CHECK(zerodds_reader_create(rt, kForbidden, "RawBytes", 0) == 0);
    CHECK(c.refused_reader >= 1);

    // 9.7: the DataTagging plugin got the tags of the writer and gave the tags
    // it announces.
    CHECK(c.set_tags >= 1);
    CHECK(c.get_tags >= 1);

    // 9.6: the refusal was logged, through the Logging plugin.
    CHECK(c.messages >= 1);
    CHECK(c.denied >= 1);

    zerodds_writer_destroy(allowed);
    zerodds_reader_destroy(reader);
    zerodds_runtime_destroy(rt);
    zerodds_security_config_destroy(cfg);

    // The runtime released each plugin: the object was deleted once, after the
    // participant config went back.
    for (int i = 0; i < 100 && (c.ac_destroyed == 0 || c.lg_destroyed == 0 || c.tg_destroyed == 0 ||
          c.au_destroyed == 0 || c.cr_destroyed == 0);
         ++i) {
        usleep(50 * 1000);
    }
    CHECK(c.ac_destroyed == 1);
    CHECK(c.participant_config_back == c.participant_config_out);
    CHECK(c.lg_destroyed == 1);
    CHECK(c.tg_destroyed == 1);
    CHECK(c.au_destroyed == 1);
    CHECK(c.cr_destroyed == 1);
    std::printf("OK\n");
    return 0;
}
