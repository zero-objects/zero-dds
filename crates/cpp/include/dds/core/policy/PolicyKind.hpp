// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/policy/PolicyKind.hpp — the kinds of the QoS policies as
// safe_enum (DDS-PSM-Cxx 1.0 §7.6.1).

#ifndef ZERODDS_DDS_CORE_POLICY_POLICYKIND_HPP
#define ZERODDS_DDS_CORE_POLICY_POLICYKIND_HPP

#include "dds/core/SafeEnumeration.hpp"

namespace dds {
namespace core {
namespace policy {

struct OwnershipKind_def {
    enum type { SHARED, EXCLUSIVE };
};
typedef dds::core::safe_enum<OwnershipKind_def> OwnershipKind;

struct DurabilityKind_def {
    enum type { VOLATILE, TRANSIENT_LOCAL, TRANSIENT, PERSISTENT };
};
typedef dds::core::safe_enum<DurabilityKind_def> DurabilityKind;

struct PresentationAccessScopeKind_def {
    enum type { INSTANCE, TOPIC, GROUP };
};
typedef dds::core::safe_enum<PresentationAccessScopeKind_def> PresentationAccessScopeKind;

struct ReliabilityKind_def {
    enum type { BEST_EFFORT, RELIABLE };
};
typedef dds::core::safe_enum<ReliabilityKind_def> ReliabilityKind;

struct DestinationOrderKind_def {
    enum type { BY_RECEPTION_TIMESTAMP, BY_SOURCE_TIMESTAMP };
};
typedef dds::core::safe_enum<DestinationOrderKind_def> DestinationOrderKind;

struct HistoryKind_def {
    enum type { KEEP_LAST, KEEP_ALL };
};
typedef dds::core::safe_enum<HistoryKind_def> HistoryKind;

struct LivelinessKind_def {
    enum type { AUTOMATIC, MANUAL_BY_PARTICIPANT, MANUAL_BY_TOPIC };
};
typedef dds::core::safe_enum<LivelinessKind_def> LivelinessKind;

/// DDS-XTypes 1.3 §7.6.3.4.1 TypeConsistencyKind. The normative header declares the enumerators in
/// a namespace, which `safe_enum` cannot take as its parameter; they are in a struct here.
struct TypeConsistencyEnforcementKind_def {
    enum type {
        EXACT_TYPE_TYPE_CONSISTENCY,
        EXACT_NAME_TYPE_CONSISTENCY,
        DECLARED_TYPE_CONSISTENCY,
        ASSIGNABLE_TYPE_CONSISTENCY
    };
};
typedef dds::core::safe_enum<TypeConsistencyEnforcementKind_def> TypeConsistencyEnforcementKind;

} // namespace policy
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_POLICY_POLICYKIND_HPP
