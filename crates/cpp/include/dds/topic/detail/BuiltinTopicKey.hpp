// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/detail/BuiltinTopicKey.hpp — storage of `BuiltinTopicKey`.

#ifndef ZERODDS_DDS_TOPIC_DETAIL_BUILTINTOPICKEY_HPP
#define ZERODDS_DDS_TOPIC_DETAIL_BUILTINTOPICKEY_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>

#include "dds/core/array.hpp"
#include "dds/topic/TBuiltinTopicKey.hpp"

namespace dds {
namespace topic {
namespace detail {

/// The 16 octets ZeroDDS identifies a discovered entity with: the GUID of the
/// participant or the endpoint (DDS-RTPS 2.5 §8.2.4.2); for a topic, the key
/// the DCPSTopic builtin topic carries. DDS 1.4 §2.2.3 declares
/// `BuiltinTopicKey_t` as three implementation-defined integers; a GUID does
/// not fit three 32-bit values, so the key keeps all sixteen octets.
class BuiltinTopicKeyDelegate {
public:
    typedef ::dds::core::array<uint8_t, 16> VALUE_T;

    BuiltinTopicKeyDelegate() : value_() { value_.fill(0); }
    explicit BuiltinTopicKeyDelegate(const VALUE_T& v) : value_(v) {}

    const VALUE_T& value() const { return value_; }

    bool operator==(const BuiltinTopicKeyDelegate& other) const { return value_ == other.value_; }
    bool operator!=(const BuiltinTopicKeyDelegate& other) const { return !(*this == other); }

private:
    VALUE_T value_;
};

typedef ::dds::topic::TBuiltinTopicKey<BuiltinTopicKeyDelegate> BuiltinTopicKey;

} // namespace detail
} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_DETAIL_BUILTINTOPICKEY_HPP
