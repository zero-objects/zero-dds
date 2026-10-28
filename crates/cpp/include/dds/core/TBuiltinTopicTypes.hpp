// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/TBuiltinTopicTypes.hpp — DDS-PSM-Cxx 1.0 (core): the built-in topic types, ready to
// create Topics, DataReaders and DataWriters for without generated code.

#ifndef ZERODDS_DDS_CORE_TBUILTINTOPICTYPES_HPP
#define ZERODDS_DDS_CORE_TBUILTINTOPICTYPES_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <string>
#include <vector>

#include "dds/core/conformance.hpp"

#if defined(OMG_DDS_X_TYPES_BUILTIN_TOPIC_TYPES_SUPPORT)

namespace dds {
namespace core {

template <typename DELEGATE>
class TBytesTopicType {
public:
    TBytesTopicType() {}
    TBytesTopicType(const std::vector<uint8_t>& data) : data_(data) {}
    operator std::vector<uint8_t>&() { return data_; }
    const std::vector<uint8_t>& data() const { return data_; }
    void data(const std::vector<uint8_t>& bytes) { data_ = bytes; }
    bool operator==(const TBytesTopicType& o) const { return data_ == o.data_; }
    bool operator!=(const TBytesTopicType& o) const { return !(*this == o); }

private:
    std::vector<uint8_t> data_;
};

template <typename DELEGATE>
class TStringTopicType {
public:
    TStringTopicType() {}
    TStringTopicType(const std::string& data) : data_(data) {}
    operator std::string&() { return data_; }
    const std::string& data() const { return data_; }
    void data(const std::string& text) { data_ = text; }
    bool operator==(const TStringTopicType& o) const { return data_ == o.data_; }
    bool operator!=(const TStringTopicType& o) const { return !(*this == o); }

private:
    std::string data_;
};

template <typename DELEGATE>
class TKeyedBytesTopicType {
public:
    TKeyedBytesTopicType() {}
    TKeyedBytesTopicType(const std::string& key, const std::vector<uint8_t>& bytes)
        : key_(key), value_(bytes) {}
    const std::string& key() const { return key_; }
    void key(const std::string& value) { key_ = value; }
    const std::vector<uint8_t>& value() const { return value_; }
    void value(const std::vector<uint8_t>& value) { value_ = value; }
    bool operator==(const TKeyedBytesTopicType& o) const {
        return key_ == o.key_ && value_ == o.value_;
    }
    bool operator!=(const TKeyedBytesTopicType& o) const { return !(*this == o); }

private:
    std::string key_;
    std::vector<uint8_t> value_;
};

template <typename DELEGATE>
class TKeyedStringTopicType {
public:
    TKeyedStringTopicType() {}
    TKeyedStringTopicType(const std::string& key, const std::string& value)
        : key_(key), value_(value) {}
    const std::string& key() const { return key_; }
    void key(const std::string& value) { key_ = value; }
    const std::string& value() const { return value_; }
    void value(const std::string& value) { value_ = value; }
    bool operator==(const TKeyedStringTopicType& o) const {
        return key_ == o.key_ && value_ == o.value_;
    }
    bool operator!=(const TKeyedStringTopicType& o) const { return !(*this == o); }

private:
    std::string key_;
    std::string value_;
};

}  // namespace core
}  // namespace dds

#endif  // OMG_DDS_X_TYPES_BUILTIN_TOPIC_TYPES_SUPPORT

#endif  // ZERODDS_DDS_CORE_TBUILTINTOPICTYPES_HPP
