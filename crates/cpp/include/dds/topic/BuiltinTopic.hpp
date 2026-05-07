// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/BuiltinTopic.hpp — DDS-PSM-Cxx 1.0 §7.5.13.4 + DDS 1.4 §2.2.5.

#ifndef ZERODDS_DDS_TOPIC_BUILTINTOPIC_HPP
#define ZERODDS_DDS_TOPIC_BUILTINTOPIC_HPP

#include <array>
#include <cstdint>
#include <string>
#include <vector>

namespace dds {
namespace topic {

/// `ParticipantBuiltinTopicData` (Spec §2.2.5.1).
class ParticipantBuiltinTopicData {
public:
    std::array<uint8_t, 16> guid{};
    std::vector<uint8_t> user_data;
};

/// `TopicBuiltinTopicData` (Spec §2.2.5.2).
class TopicBuiltinTopicData {
public:
    std::array<uint8_t, 16> key{};
    std::string name;
    std::string type_name;
    uint32_t durability_kind{0};
    uint32_t reliability_kind{0};
};

/// `PublicationBuiltinTopicData` (Spec §2.2.5.3).
class PublicationBuiltinTopicData {
public:
    std::array<uint8_t, 16> key{};
    std::array<uint8_t, 16> participant_key{};
    std::string topic_name;
    std::string type_name;
    uint32_t durability_kind{0};
    uint32_t reliability_kind{0};
    uint32_t ownership_kind{0};
    int32_t ownership_strength{0};
    int32_t liveliness_lease_seconds{0};
    int32_t deadline_seconds{0};
    int32_t lifespan_seconds{0};
};

/// `SubscriptionBuiltinTopicData` (Spec §2.2.5.4).
class SubscriptionBuiltinTopicData {
public:
    std::array<uint8_t, 16> key{};
    std::array<uint8_t, 16> participant_key{};
    std::string topic_name;
    std::string type_name;
    uint32_t durability_kind{0};
    uint32_t reliability_kind{0};
    uint32_t ownership_kind{0};
    int32_t liveliness_lease_seconds{0};
    int32_t deadline_seconds{0};
};

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_BUILTINTOPIC_HPP
