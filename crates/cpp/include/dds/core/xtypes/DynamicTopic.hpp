// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/DynamicTopic.hpp — `Topic<DynamicData>`, `DataWriter<DynamicData>` and
// `DataReader<DynamicData>`: the type of the topic is a DynamicType given at run time. The
// topic type name names it; the endpoints find its TypeObject and codec by that name.

#ifndef ZERODDS_DDS_CORE_XTYPES_DYNAMICTOPIC_HPP
#define ZERODDS_DDS_CORE_XTYPES_DYNAMICTOPIC_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <map>
#include <memory>
#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/xtypes/DynamicData.hpp"
#include "dds/topic/TopicTraits.hpp"
#include "zerodds/psm/encoding.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

/// What an endpoint of a dynamic topic needs of its type.
struct DynamicContract {
    ::dds::core::xtypes::DynamicType type;
    ::zerodds::cxx::shared_ptr<DynTypeHandle> handle;
    DynamicContract(const ::dds::core::xtypes::DynamicType& t,
                    const ::zerodds::cxx::shared_ptr<DynTypeHandle>& h)
        : type(t), handle(h) {}
};

class DynamicTypeRegistry {
public:
    static DynamicTypeRegistry& instance() {
        static DynamicTypeRegistry registry;
        return registry;
    }
    /// Makes `type` known under its topic type name; a second, different type with the same
    /// name is refused.
    std::string add(const ::dds::core::xtypes::DynamicType& type) {
        ::zerodds::cxx::shared_ptr<DynTypeHandle> handle = handle_of(type);
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
        EntryMap::iterator found = entries_.find(handle->name());
        if (found != entries_.end() && found->second->handle != handle) {
            throw ::dds::core::PreconditionNotMetError(
                "DynamicType: another type is already registered as `" + handle->name() + "`");
        }
        if (found == entries_.end()) {
            entries_[handle->name()] = ::zerodds::cxx::make_shared<DynamicContract>(type, handle);
        }
        return handle->name();
    }
    ::zerodds::cxx::shared_ptr<DynamicContract> find(const std::string& type_name) {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
        EntryMap::iterator found = entries_.find(type_name);
        if (found == entries_.end()) {
            throw ::dds::core::PreconditionNotMetError(
                "DynamicData: no DynamicType is registered as `" + type_name +
                "`; create the Topic with the DynamicType");
        }
        return found->second;
    }

private:
    ::zerodds::cxx::mutex mutex_;
    typedef std::map<std::string, ::zerodds::cxx::shared_ptr<DynamicContract> > EntryMap;
    EntryMap entries_;
};

/// The topic type name of `type`, after registering it for the endpoints.
template <typename TYPE>
inline std::string register_type(const TYPE& type) {
    return DynamicTypeRegistry::instance().add(type);
}

}  // namespace detail
}  // namespace xtypes
}  // namespace core

namespace topic {

template <>
struct topic_type_support< ::dds::core::xtypes::DynamicData> {
    typedef ::dds::core::xtypes::detail::DynamicContract Contract;

    static std::vector<uint8_t> encode(const ::dds::core::xtypes::DynamicData& v) {
        return v.encode();
    }

    /// The type of the topic whose type name is `type_name`.
    static ::zerodds::cxx::shared_ptr<Contract> contract_for(const std::string& type_name) {
        return ::dds::core::xtypes::detail::DynamicTypeRegistry::instance().find(type_name);
    }

    static ::dds::core::xtypes::DynamicData decode_for(const std::string& type_name,
                                                       const uint8_t* buf, size_t len,
                                                       ::zerodds::wire::Encoding version,
                                                       bool big_endian) {
        ::zerodds::cxx::shared_ptr<Contract> contract = contract_for(type_name);
        return ::dds::core::xtypes::DynamicData(
            ::zerodds::cxx::make_shared< ::dds::core::xtypes::detail::DynamicData>(
                contract->type, contract->handle,
                contract->handle->decode(buf, len,
                                         version == ::zerodds::wire::Encoding::PlainCdr2,
                                         big_endian)));
    }
};

}  // namespace topic
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DYNAMICTOPIC_HPP
