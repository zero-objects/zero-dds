// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/domain/DomainParticipant.hpp — DDS-PSM-Cxx 1.0 §7.5.11.

#ifndef ZERODDS_DDS_DOMAIN_DOMAINPARTICIPANT_HPP
#define ZERODDS_DDS_DOMAIN_DOMAINPARTICIPANT_HPP

#include <cstdint>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/qos.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/topic/Topic.hpp"
#include "zerodds.h"

namespace dds {
namespace domain {

/// DomainParticipant (Spec §7.5.11.5).
class DomainParticipant {
public:
    /// Konstruiert einen Participant fuer die gegebene Domain-Id mit Default-QoS.
    explicit DomainParticipant(uint32_t domain_id) {
        const zerodds_ZeroDdsDomainParticipantFactory* f = zerodds_dpf_get_instance();
        handle_ = zerodds_dpf_create_participant(f, domain_id, nullptr);
        if (handle_ == nullptr) {
            throw ::dds::core::Error("DomainParticipant::create failed");
        }
    }
    /// Konstruiert mit expliziter QoS. UserData-Bytes-Lifetime: Caller
    /// muss `qos.user_data().value()` waehrend des `create_participant`-
    /// Aufrufs am Leben halten — die C-FFI kopiert die Bytes in den
    /// Rust-Heap.
    DomainParticipant(uint32_t domain_id, const ::dds::core::DomainParticipantQos& qos) {
        const zerodds_ZeroDdsDomainParticipantFactory* f = zerodds_dpf_get_instance();
        auto native = ::dds::core::detail::to_native(qos);
        handle_ = zerodds_dpf_create_participant(f, domain_id, &native);
        if (handle_ == nullptr) {
            throw ::dds::core::Error("DomainParticipant::create with QoS failed");
        }
    }

    DomainParticipant(const DomainParticipant&) = delete;
    DomainParticipant& operator=(const DomainParticipant&) = delete;
    DomainParticipant(DomainParticipant&& o) noexcept : handle_(o.handle_) {
        o.handle_ = nullptr;
    }
    DomainParticipant& operator=(DomainParticipant&& o) noexcept {
        if (this != &o) {
            close();
            handle_ = o.handle_;
            o.handle_ = nullptr;
        }
        return *this;
    }
    ~DomainParticipant() { close(); }

    /// Domain-Id.
    uint32_t domain_id() const {
        return zerodds_dp_get_domain_id(handle_);
    }

    /// Liveliness fuer alle MANUAL_BY_PARTICIPANT-Writers asserten.
    void assert_liveliness() {
        int rc = zerodds_dp_assert_liveliness(handle_);
        ::dds::core::check_status(rc, "DomainParticipant::assert_liveliness");
    }

    /// `contains_entity` (Spec §7.5.11.5.x).
    bool contains_entity(const ::dds::core::InstanceHandle& h) const {
        return zerodds_dp_contains_entity(handle_, h.value()) != 0;
    }

    /// `ignore_participant`.
    void ignore_participant(const ::dds::core::InstanceHandle& h) {
        int rc = zerodds_dp_ignore_participant(handle_, h.value());
        ::dds::core::check_status(rc, "DomainParticipant::ignore_participant");
    }
    /// `ignore_topic`.
    void ignore_topic(const ::dds::core::InstanceHandle& h) {
        int rc = zerodds_dp_ignore_topic(handle_, h.value());
        ::dds::core::check_status(rc, "DomainParticipant::ignore_topic");
    }
    /// `ignore_publication`.
    void ignore_publication(const ::dds::core::InstanceHandle& h) {
        int rc = zerodds_dp_ignore_publication(handle_, h.value());
        ::dds::core::check_status(rc, "DomainParticipant::ignore_publication");
    }
    /// `ignore_subscription`.
    void ignore_subscription(const ::dds::core::InstanceHandle& h) {
        int rc = zerodds_dp_ignore_subscription(handle_, h.value());
        ::dds::core::check_status(rc, "DomainParticipant::ignore_subscription");
    }

    /// `delete_contained_entities`.
    void delete_contained_entities() {
        int rc = zerodds_dp_delete_contained_entities(handle_);
        ::dds::core::check_status(rc, "DomainParticipant::delete_contained_entities");
    }

    /// `get_discovered_participants` (Spec §7.5.11.5.x).
    ::dds::core::InstanceHandleSeq get_discovered_participants() const {
        std::vector<uint64_t> raw(64, 0);
        size_t count = 0;
        int rc = zerodds_dp_get_discovered_participants(handle_, raw.data(), &count, raw.size());
        ::dds::core::check_status(rc, "DomainParticipant::get_discovered_participants");
        ::dds::core::InstanceHandleSeq out;
        out.reserve(count);
        for (size_t i = 0; i < count; ++i) {
            out.emplace_back(raw[i]);
        }
        return out;
    }

    /// Native Handle (fuer Topic/Pub/Sub-Konstruktion).
    zerodds_ZeroDdsDomainParticipant* native_handle() const { return handle_; }

private:
    void close() {
        if (handle_ != nullptr) {
            const zerodds_ZeroDdsDomainParticipantFactory* f = zerodds_dpf_get_instance();
            zerodds_dp_delete_contained_entities(handle_);
            zerodds_dpf_delete_participant(f, handle_);
            handle_ = nullptr;
        }
    }
    zerodds_ZeroDdsDomainParticipant* handle_{nullptr};
};

/// DomainParticipantFactory (Spec §7.5.11.4). Singleton.
class DomainParticipantFactory {
public:
    /// Erzeugt Participant.
    static DomainParticipant create_participant(uint32_t domain_id) {
        return DomainParticipant(domain_id);
    }
    /// Erzeugt Participant mit QoS.
    static DomainParticipant create_participant(uint32_t domain_id,
                                                const ::dds::core::DomainParticipantQos& qos) {
        return DomainParticipant(domain_id, qos);
    }
};

} // namespace domain
} // namespace dds

// Topic-Constructor-Implementations (forward-deklariert in Topic.hpp).
namespace dds {
namespace topic {

template <typename T>
Topic<T>::Topic(::dds::domain::DomainParticipant& dp, const std::string& name)
    : TopicDescription(zerodds_dp_create_topic(dp.native_handle(), name.c_str(),
                                               type_support::type_name(), nullptr)),
      participant_(dp.native_handle()) {
    if (handle_ == nullptr) {
        throw ::dds::core::Error("Topic::create failed");
    }
}

template <typename T>
Topic<T>::Topic(::dds::domain::DomainParticipant& dp, const std::string& name,
                const ::dds::core::TopicQos& qos)
    : TopicDescription(nullptr), participant_(dp.native_handle()) {
    auto native = ::dds::core::detail::to_native(qos);
    handle_ = zerodds_dp_create_topic(participant_, name.c_str(),
                                      type_support::type_name(), &native);
    if (handle_ == nullptr) {
        throw ::dds::core::Error("Topic::create with QoS failed");
    }
}

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_DOMAIN_DOMAINPARTICIPANT_HPP
