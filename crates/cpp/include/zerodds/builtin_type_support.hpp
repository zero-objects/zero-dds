// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds/builtin_type_support.hpp — the TypeSupport interfaces of DDS-XTypes 1.3 Annex E:
// `StringTypeSupport`, `KeyedStringTypeSupport`, `BytesTypeSupport` and `KeyedBytesTypeSupport`,
// each "an instantiation of FooTypeSupport defined by the DDS specification" (DDS 1.4
// §2.2.2.3.6): `get_type_name` and `register_type`.
//
//     DDS::StringTypeSupport support;
//     support.register_type(participant);               // under "DDS::String"
//     support.register_type(participant, "Greetings");  // and under another name
//
// A participant knows the four types under their own names from its creation (§7.6.5);
// registering one under a name of the caller's makes a topic of that type creatable by the name.

#ifndef ZERODDS_BUILTIN_TYPE_SUPPORT_HPP
#define ZERODDS_BUILTIN_TYPE_SUPPORT_HPP

#include <stdint.h>

#include "dds/domain/DomainParticipant.hpp"
#include "dds/types.hpp"
#include "zerodds.h"

namespace DDS {

/// The base of the type supports (DDS 1.4 §2.2.2.3.6).
class TypeSupport {
public:
    virtual ~TypeSupport() = default;

    /// The name the type is registered under unless `register_type` is given another.
    virtual const char* get_type_name() const = 0;

    /// Registers the type with `participant` under `type_name`, or under `get_type_name()`
    /// where it is null. RETCODE_PRECONDITION_NOT_MET when another type has the name,
    /// RETCODE_BAD_PARAMETER for a name that is none, RETCODE_ALREADY_DELETED.
    virtual ReturnCode_t register_type(dds::domain::DomainParticipant& participant,
                                       const char* type_name = nullptr) const = 0;
};

namespace detail {

/// The `ZERODDS_BUILTIN_*` kind of the C ABI is the template argument.
template <int Kind>
class BuiltinTypeSupport : public TypeSupport {
public:
    const char* get_type_name() const override {
        return zerodds_builtin_type_support_get_type_name(Kind);
    }

    ReturnCode_t register_type(dds::domain::DomainParticipant& participant,
                               const char* type_name = nullptr) const override {
        zerodds_ZeroDdsDomainParticipant* handle = participant.delegate()->native_handle();
        switch (zerodds_dp_register_builtin_type(handle, Kind, type_name)) {
            case 0:
                return RETCODE_OK;
            case -2:  // a null handle
            case -3:  // a name that is no text
            case -6:
                return RETCODE_BAD_PARAMETER;
            case -5:
                return RETCODE_PRECONDITION_NOT_MET;
            case -12:
                return RETCODE_ALREADY_DELETED;
            default:
                return RETCODE_ERROR;
        }
    }
};

}  // namespace detail

/// `DDS::String` (Annex E).
typedef detail::BuiltinTypeSupport<0> StringTypeSupport;
/// `DDS::KeyedString`.
typedef detail::BuiltinTypeSupport<1> KeyedStringTypeSupport;
/// `DDS::Bytes`.
typedef detail::BuiltinTypeSupport<2> BytesTypeSupport;
/// `DDS::KeyedBytes`.
typedef detail::BuiltinTypeSupport<3> KeyedBytesTypeSupport;

}  // namespace DDS

#endif  // ZERODDS_BUILTIN_TYPE_SUPPORT_HPP
