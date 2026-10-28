// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/TBuiltinTopicKey.hpp — DDS-PSM-Cxx 1.0 `TBuiltinTopicKey<D>`.

#ifndef ZERODDS_DDS_TOPIC_TBUILTINTOPICKEY_HPP
#define ZERODDS_DDS_TOPIC_TBUILTINTOPICKEY_HPP

#include "dds/core/Value.hpp"

namespace dds {
namespace topic {

/// The key of a discovered entity.
template <typename D>
class TBuiltinTopicKey : public ::dds::core::Value<D> {
public:
    typedef typename D::VALUE_T VALUE_T;

    TBuiltinTopicKey() : ::dds::core::Value<D>() {}
    explicit TBuiltinTopicKey(const VALUE_T& v) : ::dds::core::Value<D>(v) {}

    /// The key's value.
    VALUE_T value() const { return this->delegate().value(); }
};

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_TBUILTINTOPICKEY_HPP
