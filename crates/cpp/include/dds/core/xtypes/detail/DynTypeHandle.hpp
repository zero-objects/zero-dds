// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/xtypes/detail/DynTypeHandle.hpp — a dynamic type as the C API holds it: the model
// the IDL text lowered to, with the TypeObject, the key and the codec behind it.

#ifndef ZERODDS_DDS_CORE_XTYPES_DETAIL_DYNTYPEHANDLE_HPP
#define ZERODDS_DDS_CORE_XTYPES_DETAIL_DYNTYPEHANDLE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <memory>
#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/xtypes/detail/Json.hpp"
#include "zerodds.h"
#include "zerodds/psm/check_status.hpp"

namespace dds {
namespace core {
namespace xtypes {
namespace detail {

class DynTypeHandle {
public:
    /// The type `type_name` that `idl` declares.
    DynTypeHandle(const std::string& idl, const std::string& type_name)
        : handle_(0), name_(), keyed_(false), extensibility_(1), type_object_(), dependencies_() {
        int status = 0;
        handle_ = zerodds_dyntype_from_idl(idl.c_str(), type_name.c_str(), &status);
        if (handle_ == 0) {
            throw ::dds::core::InvalidArgumentError(
                "DynamicType: the C API refused the type `" + type_name + "`, written as IDL:\n" + idl);
        }
        char* name = 0;
        ::zerodds::psm::check_status(zerodds_dyntype_name(handle_, &name), "DynamicType::name");
        name_ = name;
        zerodds_string_free(name);
        keyed_ = zerodds_dyntype_is_keyed(handle_) > 0;
        extensibility_ = zerodds_dyntype_extensibility(handle_);
        std::size_t len = 0;
        const uint8_t* object = zerodds_dyntype_type_object(handle_, &len);
        if (object != 0) {
            type_object_.assign(object, object + len);
        }
        const std::size_t count = zerodds_dyntype_dependency_count(handle_);
        for (std::size_t i = 0; i < count; ++i) {
            std::size_t dep_len = 0;
            const uint8_t* dep = zerodds_dyntype_dependency(handle_, i, &dep_len);
            if (dep != 0) {
                dependencies_.push_back(std::vector<uint8_t>(dep, dep + dep_len));
            }
        }
    }
    ~DynTypeHandle() { zerodds_dyntype_destroy(handle_); }

    ZERODDS_NONCOPYABLE(DynTypeHandle)

    /// The topic type name: the scoped name of the type.
    const std::string& name() const { return name_; }
    bool keyed() const { return keyed_; }
    /// 0 FINAL, 1 APPENDABLE, 2 MUTABLE.
    int extensibility() const { return extensibility_; }
    const std::vector<uint8_t>& type_object() const { return type_object_; }
    const std::vector<std::vector<uint8_t> >& dependencies() const { return dependencies_; }

    /// The default value of the type (XTypes 1.3 Table 9).
    Json default_value() const {
        char* text = 0;
        ::zerodds::psm::check_status(zerodds_dyntype_default_json(handle_, &text),
                                     "DynamicData: the default value");
        Json value = Json::parse(text);
        zerodds_string_free(text);
        return value;
    }

    /// The body of a sample for `value`. `InvalidDataError` when `value` is not a value of the
    /// type — a member missing, a number out of range.
    std::vector<uint8_t> encode(const Json& value, bool xcdr2, bool big_endian) const {
        uint8_t* bytes = 0;
        std::size_t len = 0;
        const int rc = zerodds_dyntype_encode_json(handle_, value.dump().c_str(), xcdr2 ? 1 : 0,
                                                   big_endian ? 1 : 0, &bytes, &len);
        if (rc != 0) {
            throw ::dds::core::InvalidDataError("DynamicData: the value is not a value of the type " +
                                                name_);
        }
        std::vector<uint8_t> out(bytes, bytes + len);
        zerodds_dyntype_bytes_free(bytes, len);
        return out;
    }

    /// The value the body of a sample holds.
    Json decode(const uint8_t* bytes, std::size_t len, bool xcdr2, bool big_endian) const {
        char* text = 0;
        const int rc = zerodds_dyntype_decode_json(handle_, bytes, len, xcdr2 ? 1 : 0,
                                                   big_endian ? 1 : 0, &text);
        if (rc != 0) {
            throw ::dds::core::InvalidDataError("DynamicData: the octets are not a value of the type " +
                                                name_);
        }
        Json value = Json::parse(text);
        zerodds_string_free(text);
        return value;
    }

private:
    zerodds_ZeroDdsDynType* handle_;
    std::string name_;
    bool keyed_;
    int extensibility_;
    std::vector<uint8_t> type_object_;
    std::vector<std::vector<uint8_t> > dependencies_;
};

}  // namespace detail
}  // namespace xtypes
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_XTYPES_DETAIL_DYNTYPEHANDLE_HPP
