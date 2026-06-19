// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/Topic.hpp — DDS-PSM-Cxx 1.0 §7.5.13.

#ifndef ZERODDS_DDS_TOPIC_TOPIC_HPP
#define ZERODDS_DDS_TOPIC_TOPIC_HPP

#include <string>

#include "dds/core/Exception.hpp"
#include "dds/core/qos.hpp"
#include "dds/topic/TopicTraits.hpp"
#include "zerodds.h"

namespace dds {
namespace domain {
class DomainParticipant;
}

namespace topic {

/// TopicDescription (Spec §7.5.13.1).
class TopicDescription {
public:
    TopicDescription() = default;
    explicit TopicDescription(zerodds_ZeroDdsTopic* h) : handle_(h) {}
    /// C-FFI Native Handle.
    zerodds_ZeroDdsTopic* native_handle() const { return handle_; }
    /// Topic-Name.
    std::string name() const {
        if (handle_ == nullptr) return {};
        char* raw = zerodds_topic_get_name(handle_);
        if (raw == nullptr) return {};
        std::string out(raw);
        zerodds_string_free(raw);
        return out;
    }
    /// Type-Name.
    std::string type_name() const {
        if (handle_ == nullptr) return {};
        char* raw = zerodds_topic_get_type_name(handle_);
        if (raw == nullptr) return {};
        std::string out(raw);
        zerodds_string_free(raw);
        return out;
    }
protected:
    zerodds_ZeroDdsTopic* handle_{nullptr};
};

/// Topic<T> (Spec §7.5.13.5).
template <typename T>
class Topic : public TopicDescription {
public:
    /// Default-konstruiert ein leeres Topic-Handle.
    Topic() = default;
    /// Constructs a topic via Participant + Name. QoS = default.
    Topic(::dds::domain::DomainParticipant& dp, const std::string& name);
    /// Constructs a topic via Participant + Name + QoS.
    Topic(::dds::domain::DomainParticipant& dp, const std::string& name,
          const ::dds::core::TopicQos& qos);

    Topic(const Topic&) = delete;
    Topic& operator=(const Topic&) = delete;
    Topic(Topic&& o) noexcept : TopicDescription(o.handle_), participant_(o.participant_) {
        o.handle_ = nullptr;
        o.participant_ = nullptr;
    }
    Topic& operator=(Topic&& o) noexcept {
        if (this != &o) {
            handle_ = o.handle_;
            participant_ = o.participant_;
            o.handle_ = nullptr;
            o.participant_ = nullptr;
        }
        return *this;
    }
    ~Topic() {
        if (handle_ != nullptr && participant_ != nullptr) {
            zerodds_dp_delete_topic(participant_, handle_);
        }
    }

    /// Type-Support trait.
    using type_support = topic_type_support<T>;

private:
    zerodds_ZeroDdsDomainParticipant* participant_{nullptr};
    template <typename U>
    friend class ::dds::topic::Topic;
};

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_TOPIC_HPP
