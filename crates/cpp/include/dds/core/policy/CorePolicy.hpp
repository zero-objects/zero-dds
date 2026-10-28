// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/policy/CorePolicy.hpp — the QoS policy classes (DDS-PSM-Cxx 1.0
// §7.6.1). Kinds are safe_enum values (`HistoryKind::KEEP_LAST`), a kind
// has a static helper per variant (`History::KeepLast(10)`), and every
// attribute has a getter, a reference getter and a chainable setter.

#ifndef ZERODDS_DDS_CORE_POLICY_COREPOLICY_HPP
#define ZERODDS_DDS_CORE_POLICY_COREPOLICY_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <string>
#include <utility>
#include <vector>

#include "dds/core/Duration.hpp"
#include "dds/core/LengthUnlimited.hpp"
#include "dds/core/policy/PolicyKind.hpp"
#include "dds/core/policy/TCorePolicy.hpp"
#include "dds/core/policy/QosPolicyCount.hpp"
#include "dds/core/types.hpp"

// The defaulted copy and move operations of a policy class (C++11); C++03 has the implicit copy.
#if ZERODDS_CXX11
#define ZERODDS_POLICY_DEFAULTS(CLS)                   \
    CLS(const CLS& other) = default;                   \
    CLS(CLS&&) = default;                              \
    CLS& operator=(const CLS& other) = default;        \
    CLS& operator=(CLS&&) = default;
#else
#define ZERODDS_POLICY_DEFAULTS(CLS)
#endif

// An attribute of a native type: `T name() const`, `T& name()`, `P& name(T)`.
#define ZERODDS_POLICY_NT(CLS, T, name)                \
    T name() const { return name##_; }                 \
    T& name() { return name##_; }                      \
    CLS& name(T value) {                               \
        name##_ = value;                               \
        return *this;                                  \
    }
// An attribute of a constructed type: `T name() const` (by value, as the normative headers
// declare it), `T& name()`, `P& name(const T&)`.
#define ZERODDS_POLICY_CT(CLS, T, name)                \
    T name() const { return name##_; }                 \
    T& name() { return name##_; }                      \
    CLS& name(const T& value) {                        \
        name##_ = value;                               \
        return *this;                                  \
    }

namespace dds {
namespace core {
namespace policy {

// ---- Policy trait classes (§7.6.1) -------------------------------------

template <typename Policy>
class policy_id {};
template <typename Policy>
class policy_name {};

namespace detail {

// ---- Data policies ------------------------------------------------------

#define ZERODDS_DATA_POLICY(CLS, DOC)                                                        \
    class CLS {                                                                              \
    public:                                                                                  \
        CLS() {}                                                                             \
        explicit CLS(const dds::core::ByteSeq& seq) : value_(seq) {}                         \
        CLS(const uint8_t* value_begin, const uint8_t* value_end)                            \
            : value_(value_begin, value_end) {}                                              \
        ZERODDS_POLICY_DEFAULTS(CLS)                                                         \
                                                                                             \
        CLS& value(const dds::core::ByteSeq& seq) {                                          \
            value_ = seq;                                                                    \
            return *this;                                                                    \
        }                                                                                    \
        template <typename OCTET_ITER>                                                       \
        CLS& value(OCTET_ITER begin, OCTET_ITER end) {                                       \
            value_.assign(begin, end);                                                       \
            return *this;                                                                    \
        }                                                                                    \
        const dds::core::ByteSeq& value() const { return value_; }                           \
        dds::core::ByteSeq& value() { return value_; }                                       \
        const uint8_t* begin() const {                                                       \
            return value_.empty() ? static_cast<const uint8_t*>(0) : &value_[0];             \
        }                                                                                    \
        const uint8_t* end() const { return begin() + value_.size(); }                       \
                                                                                             \
    private:                                                                                 \
        dds::core::ByteSeq value_;                                                           \
    };

ZERODDS_DATA_POLICY(UserData, USER_DATA)
ZERODDS_DATA_POLICY(GroupData, GROUP_DATA)
ZERODDS_DATA_POLICY(TopicData, TOPIC_DATA)
#undef ZERODDS_DATA_POLICY

/// ENTITY_FACTORY.
class EntityFactory {
public:
    EntityFactory() : autoenable_created_entities_(true) {}
    explicit EntityFactory(bool the_auto_enable) : autoenable_created_entities_(the_auto_enable) {}
#if ZERODDS_CXX11
    EntityFactory(const EntityFactory& other) = default;
    EntityFactory(EntityFactory&&) = default;
    EntityFactory& operator=(const EntityFactory& other) = default;
    EntityFactory& operator=(EntityFactory&&) = default;
#endif

    ZERODDS_POLICY_NT(EntityFactory, bool, autoenable_created_entities)

    static EntityFactory AutoEnable() { return EntityFactory(true); }
    static EntityFactory ManuallyEnable() { return EntityFactory(false); }

private:
    bool autoenable_created_entities_;
};

/// TRANSPORT_PRIORITY.
class TransportPriority {
public:
    TransportPriority() : value_(0) {}
    explicit TransportPriority(int32_t prio) : value_(prio) {}
#if ZERODDS_CXX11
    TransportPriority(const TransportPriority& other) = default;
    TransportPriority(TransportPriority&&) = default;
    TransportPriority& operator=(const TransportPriority& other) = default;
    TransportPriority& operator=(TransportPriority&&) = default;
#endif

    ZERODDS_POLICY_NT(TransportPriority, int32_t, value)

private:
    int32_t value_;
};

// ---- Policies of one Duration ------------------------------------------

#define ZERODDS_DURATION_POLICY(CLS, attr, DEFAULT)                                \
    class CLS {                                                                    \
    public:                                                                        \
        CLS() : attr##_(DEFAULT) {}                                                \
        explicit CLS(const dds::core::Duration& d) : attr##_(d) {}                 \
        ZERODDS_POLICY_DEFAULTS(CLS)                                               \
                                                                                   \
        ZERODDS_POLICY_CT(CLS, dds::core::Duration, attr)                          \
                                                                                   \
    private:                                                                       \
        dds::core::Duration attr##_;                                               \
    };

/// LIFESPAN.
ZERODDS_DURATION_POLICY(Lifespan, duration, dds::core::Duration::infinite())
/// DEADLINE.
ZERODDS_DURATION_POLICY(Deadline, period, dds::core::Duration::infinite())
/// LATENCY_BUDGET.
ZERODDS_DURATION_POLICY(LatencyBudget, duration, dds::core::Duration::zero())
/// TIME_BASED_FILTER.
ZERODDS_DURATION_POLICY(TimeBasedFilter, minimum_separation, dds::core::Duration::zero())
#undef ZERODDS_DURATION_POLICY

/// PARTITION.
class Partition {
public:
    Partition() {}
    explicit Partition(const std::string& partition) : name_(1, partition) {}
    explicit Partition(const dds::core::StringSeq& partitions) : name_(partitions) {}
#if ZERODDS_CXX11
    Partition(const Partition& other) = default;
    Partition(Partition&&) = default;
    Partition& operator=(const Partition& other) = default;
    Partition& operator=(Partition&&) = default;
#endif

    Partition& name(const dds::core::StringSeq& partitions) {
        name_ = partitions;
        return *this;
    }
    const dds::core::StringSeq& name() const { return name_; }
    dds::core::StringSeq& name() { return name_; }

private:
    dds::core::StringSeq name_;
};

// ---- Ownership ---------------------------------------------------------

/// OWNERSHIP.
class Ownership {
public:
    Ownership() : kind_(OwnershipKind::SHARED) {}
    explicit Ownership(OwnershipKind::type the_kind) : kind_(the_kind) {}
#if ZERODDS_CXX11
    Ownership(const Ownership& other) = default;
    Ownership(Ownership&&) = default;
    Ownership& operator=(const Ownership& other) = default;
    Ownership& operator=(Ownership&&) = default;
#endif

    Ownership& kind(OwnershipKind::type the_kind) {
        kind_ = the_kind;
        return *this;
    }
    OwnershipKind::type kind() const { return kind_; }
    OwnershipKind::type& kind() { return kind_; }

    static Ownership Exclusive() { return Ownership(OwnershipKind::EXCLUSIVE); }
    static Ownership Shared() { return Ownership(OwnershipKind::SHARED); }

private:
    OwnershipKind::type kind_;
};

/// OWNERSHIP_STRENGTH.
class OwnershipStrength {
public:
    OwnershipStrength() : value_(0) {}
    explicit OwnershipStrength(int32_t s) : value_(s) {}
#if ZERODDS_CXX11
    OwnershipStrength(const OwnershipStrength& other) = default;
    OwnershipStrength(OwnershipStrength&&) = default;
    OwnershipStrength& operator=(const OwnershipStrength& other) = default;
    OwnershipStrength& operator=(OwnershipStrength&&) = default;
#endif

    ZERODDS_POLICY_NT(OwnershipStrength, int32_t, value)

private:
    int32_t value_;
};

// ---- Lifecycle ----------------------------------------------------------

/// WRITER_DATA_LIFECYCLE.
class WriterDataLifecycle {
public:
    WriterDataLifecycle() : autodispose_unregistered_instances_(true) {}
    explicit WriterDataLifecycle(bool the_autodispose)
        : autodispose_unregistered_instances_(the_autodispose) {}
#if ZERODDS_CXX11
    WriterDataLifecycle(const WriterDataLifecycle& other) = default;
    WriterDataLifecycle(WriterDataLifecycle&&) = default;
    WriterDataLifecycle& operator=(const WriterDataLifecycle& other) = default;
    WriterDataLifecycle& operator=(WriterDataLifecycle&&) = default;
#endif

    ZERODDS_POLICY_NT(WriterDataLifecycle, bool, autodispose_unregistered_instances)

    static WriterDataLifecycle AutoDisposeUnregisteredInstances() {
        return WriterDataLifecycle(true);
    }
    static WriterDataLifecycle ManuallyDisposeUnregisteredInstances() {
        return WriterDataLifecycle(false);
    }

private:
    bool autodispose_unregistered_instances_;
};

/// READER_DATA_LIFECYCLE.
class ReaderDataLifecycle {
public:
    ReaderDataLifecycle()
        : autopurge_nowriter_samples_delay_(dds::core::Duration::infinite()),
          autopurge_disposed_samples_delay_(dds::core::Duration::infinite()) {}
    ReaderDataLifecycle(const dds::core::Duration& the_nowriter_delay,
                        const dds::core::Duration& the_disposed_samples_delay)
        : autopurge_nowriter_samples_delay_(the_nowriter_delay),
          autopurge_disposed_samples_delay_(the_disposed_samples_delay) {}
#if ZERODDS_CXX11
    ReaderDataLifecycle(const ReaderDataLifecycle& other) = default;
    ReaderDataLifecycle(ReaderDataLifecycle&&) = default;
    ReaderDataLifecycle& operator=(const ReaderDataLifecycle& other) = default;
    ReaderDataLifecycle& operator=(ReaderDataLifecycle&&) = default;
#endif

    ZERODDS_POLICY_CT(ReaderDataLifecycle, dds::core::Duration, autopurge_nowriter_samples_delay)
    ZERODDS_POLICY_CT(ReaderDataLifecycle, dds::core::Duration, autopurge_disposed_samples_delay)

    static ReaderDataLifecycle NoAutoPurgeDisposedSamples() { return ReaderDataLifecycle(); }
    static ReaderDataLifecycle AutoPurgeDisposedSamples(const dds::core::Duration& d) {
        return ReaderDataLifecycle(dds::core::Duration::infinite(), d);
    }

private:
    dds::core::Duration autopurge_nowriter_samples_delay_;
    dds::core::Duration autopurge_disposed_samples_delay_;
};

// ---- Policies with a kind ----------------------------------------------

/// DURABILITY.
class Durability {
public:
    Durability() : kind_(DurabilityKind::VOLATILE) {}
    explicit Durability(DurabilityKind::type the_kind) : kind_(the_kind) {}
#if ZERODDS_CXX11
    Durability(const Durability& other) = default;
    Durability(Durability&&) = default;
    Durability& operator=(const Durability& other) = default;
    Durability& operator=(Durability&&) = default;
#endif

    Durability& kind(DurabilityKind::type the_kind) {
        kind_ = the_kind;
        return *this;
    }
    DurabilityKind::type kind() const { return kind_; }
    DurabilityKind::type& kind() { return kind_; }

    static Durability Volatile() { return Durability(DurabilityKind::VOLATILE); }
    static Durability TransientLocal() { return Durability(DurabilityKind::TRANSIENT_LOCAL); }
    static Durability Transient() { return Durability(DurabilityKind::TRANSIENT); }
    static Durability Persistent() { return Durability(DurabilityKind::PERSISTENT); }

private:
    DurabilityKind::type kind_;
};

/// PRESENTATION.
class Presentation {
public:
    Presentation()
        : access_scope_(PresentationAccessScopeKind::INSTANCE),
          coherent_access_(false),
          ordered_access_(false) {}
    Presentation(PresentationAccessScopeKind::type the_access_scope, bool the_coherent_access,
                 bool the_ordered_access)
        : access_scope_(the_access_scope),
          coherent_access_(the_coherent_access),
          ordered_access_(the_ordered_access) {}
#if ZERODDS_CXX11
    Presentation(const Presentation& other) = default;
    Presentation(Presentation&&) = default;
    Presentation& operator=(const Presentation& other) = default;
    Presentation& operator=(Presentation&&) = default;
#endif

    Presentation& access_scope(PresentationAccessScopeKind::type as) {
        access_scope_ = as;
        return *this;
    }
    PresentationAccessScopeKind::type access_scope() const { return access_scope_; }
    PresentationAccessScopeKind::type& access_scope() { return access_scope_; }
    ZERODDS_POLICY_NT(Presentation, bool, coherent_access)
    ZERODDS_POLICY_NT(Presentation, bool, ordered_access)

    static Presentation GroupAccessScope(bool coherent = false, bool ordered = false) {
        return Presentation(PresentationAccessScopeKind::GROUP, coherent, ordered);
    }
    static Presentation InstanceAccessScope(bool coherent = false, bool ordered = false) {
        return Presentation(PresentationAccessScopeKind::INSTANCE, coherent, ordered);
    }
    static Presentation TopicAccessScope(bool coherent = false, bool ordered = false) {
        return Presentation(PresentationAccessScopeKind::TOPIC, coherent, ordered);
    }

private:
    PresentationAccessScopeKind::type access_scope_;
    bool coherent_access_;
    bool ordered_access_;
};

/// RELIABILITY.
class Reliability {
public:
    Reliability()
        : kind_(ReliabilityKind::BEST_EFFORT),
          max_blocking_time_(dds::core::Duration::from_millisecs(100)) {}
    Reliability(ReliabilityKind::type the_kind, const dds::core::Duration& the_max_blocking_time)
        : kind_(the_kind),
          max_blocking_time_(the_max_blocking_time) {}
#if ZERODDS_CXX11
    Reliability(const Reliability& other) = default;
    Reliability(Reliability&&) = default;
    Reliability& operator=(const Reliability& other) = default;
    Reliability& operator=(Reliability&&) = default;
#endif

    Reliability& kind(ReliabilityKind::type the_kind) {
        kind_ = the_kind;
        return *this;
    }
    ReliabilityKind::type kind() const { return kind_; }
    ReliabilityKind::type& kind() { return kind_; }
    ZERODDS_POLICY_CT(Reliability, dds::core::Duration, max_blocking_time)

    static Reliability Reliable(const dds::core::Duration& d = dds::core::Duration::infinite()) {
        return Reliability(ReliabilityKind::RELIABLE, d);
    }
    static Reliability BestEffort() {
        return Reliability(ReliabilityKind::BEST_EFFORT, dds::core::Duration::from_millisecs(100));
    }

private:
    // DDS 1.4 §2.2.3: BEST_EFFORT for a Topic and a DataReader, RELIABLE for
    // a DataWriter; `DataWriterQos` names its own.
    ReliabilityKind::type kind_;
    dds::core::Duration max_blocking_time_;
};

/// DESTINATION_ORDER.
class DestinationOrder {
public:
    DestinationOrder() : kind_(DestinationOrderKind::BY_RECEPTION_TIMESTAMP) {}
    explicit DestinationOrder(DestinationOrderKind::type the_kind) : kind_(the_kind) {}
#if ZERODDS_CXX11
    DestinationOrder(const DestinationOrder& other) = default;
    DestinationOrder(DestinationOrder&&) = default;
    DestinationOrder& operator=(const DestinationOrder& other) = default;
    DestinationOrder& operator=(DestinationOrder&&) = default;
#endif

    DestinationOrder& kind(DestinationOrderKind::type the_kind) {
        kind_ = the_kind;
        return *this;
    }
    DestinationOrderKind::type kind() const { return kind_; }
    DestinationOrderKind::type& kind() { return kind_; }

    static DestinationOrder SourceTimestamp() {
        return DestinationOrder(DestinationOrderKind::BY_SOURCE_TIMESTAMP);
    }
    static DestinationOrder ReceptionTimestamp() {
        return DestinationOrder(DestinationOrderKind::BY_RECEPTION_TIMESTAMP);
    }

private:
    DestinationOrderKind::type kind_;
};

/// HISTORY.
class History {
public:
    History() : kind_(HistoryKind::KEEP_LAST), depth_(1) {}
    History(HistoryKind::type the_kind, int32_t the_depth) : kind_(the_kind), depth_(the_depth) {}
#if ZERODDS_CXX11
    History(const History& other) = default;
    History(History&&) = default;
    History& operator=(const History& other) = default;
    History& operator=(History&&) = default;
#endif

    HistoryKind::type kind() const { return kind_; }
    HistoryKind::type& kind() { return kind_; }
    History& kind(HistoryKind::type the_kind) {
        kind_ = the_kind;
        return *this;
    }
    ZERODDS_POLICY_NT(History, int32_t, depth)

    static History KeepAll() { return History(HistoryKind::KEEP_ALL, dds::core::LENGTH_UNLIMITED); }
    static History KeepLast(uint32_t depth) {
        return History(HistoryKind::KEEP_LAST, static_cast<int32_t>(depth));
    }

private:
    HistoryKind::type kind_;
    int32_t depth_;
};

/// RESOURCE_LIMITS. The default is LENGTH_UNLIMITED in all three members
/// (DDS 1.4 §2.2.3.19).
class ResourceLimits {
public:
    ResourceLimits()
        : max_samples_(dds::core::LENGTH_UNLIMITED),
          max_instances_(dds::core::LENGTH_UNLIMITED),
          max_samples_per_instance_(dds::core::LENGTH_UNLIMITED) {}
    ResourceLimits(int32_t the_max_samples, int32_t the_max_instances,
                   int32_t the_max_samples_per_instance)
        : max_samples_(the_max_samples),
          max_instances_(the_max_instances),
          max_samples_per_instance_(the_max_samples_per_instance) {}
#if ZERODDS_CXX11
    ResourceLimits(const ResourceLimits& other) = default;
    ResourceLimits(ResourceLimits&&) = default;
    ResourceLimits& operator=(const ResourceLimits& other) = default;
    ResourceLimits& operator=(ResourceLimits&&) = default;
#endif

    ZERODDS_POLICY_NT(ResourceLimits, int32_t, max_samples)
    ZERODDS_POLICY_NT(ResourceLimits, int32_t, max_instances)
    ZERODDS_POLICY_NT(ResourceLimits, int32_t, max_samples_per_instance)

private:
    int32_t max_samples_;
    int32_t max_instances_;
    int32_t max_samples_per_instance_;
};

/// LIVELINESS.
class Liveliness {
public:
    Liveliness() : kind_(LivelinessKind::AUTOMATIC), lease_duration_(dds::core::Duration::infinite()) {}
    Liveliness(LivelinessKind::type the_kind, const dds::core::Duration& the_lease_duration)
        : kind_(the_kind),
          lease_duration_(the_lease_duration) {}
#if ZERODDS_CXX11
    Liveliness(const Liveliness& other) = default;
    Liveliness(Liveliness&&) = default;
    Liveliness& operator=(const Liveliness& other) = default;
    Liveliness& operator=(Liveliness&&) = default;
#endif

    Liveliness& kind(LivelinessKind::type the_kind) {
        kind_ = the_kind;
        return *this;
    }
    LivelinessKind::type kind() const { return kind_; }
    LivelinessKind::type& kind() { return kind_; }
    ZERODDS_POLICY_CT(Liveliness, dds::core::Duration, lease_duration)

    static Liveliness Automatic() {
        return Liveliness(LivelinessKind::AUTOMATIC, dds::core::Duration::infinite());
    }
    static Liveliness ManualByParticipant(
        const dds::core::Duration& lease = dds::core::Duration::infinite()) {
        return Liveliness(LivelinessKind::MANUAL_BY_PARTICIPANT, lease);
    }
    static Liveliness ManualByTopic(
        const dds::core::Duration& lease = dds::core::Duration::infinite()) {
        return Liveliness(LivelinessKind::MANUAL_BY_TOPIC, lease);
    }

private:
    LivelinessKind::type kind_;
    dds::core::Duration lease_duration_;
};

/// DURABILITY_SERVICE.
class DurabilityService {
public:
    DurabilityService()
        : service_cleanup_delay_(dds::core::Duration::zero()),
          history_kind_(HistoryKind::KEEP_LAST),
          history_depth_(1),
          max_samples_(dds::core::LENGTH_UNLIMITED),
          max_instances_(dds::core::LENGTH_UNLIMITED),
          max_samples_per_instance_(dds::core::LENGTH_UNLIMITED) {}
    DurabilityService(const dds::core::Duration& the_service_cleanup_delay,
                      HistoryKind::type the_history_kind, int32_t the_history_depth,
                      int32_t the_max_samples, int32_t the_max_instances,
                      int32_t the_max_samples_per_instance)
        : service_cleanup_delay_(the_service_cleanup_delay),
          history_kind_(the_history_kind),
          history_depth_(the_history_depth),
          max_samples_(the_max_samples),
          max_instances_(the_max_instances),
          max_samples_per_instance_(the_max_samples_per_instance) {}
#if ZERODDS_CXX11
    DurabilityService(const DurabilityService& other) = default;
    DurabilityService(DurabilityService&&) = default;
    DurabilityService& operator=(const DurabilityService& other) = default;
    DurabilityService& operator=(DurabilityService&&) = default;
#endif

    ZERODDS_POLICY_CT(DurabilityService, dds::core::Duration, service_cleanup_delay)
    DurabilityService& history_kind(HistoryKind::type the_kind) {
        history_kind_ = the_kind;
        return *this;
    }
    HistoryKind::type history_kind() const { return history_kind_; }
    HistoryKind::type& history_kind() { return history_kind_; }
    ZERODDS_POLICY_NT(DurabilityService, int32_t, history_depth)
    ZERODDS_POLICY_NT(DurabilityService, int32_t, max_samples)
    ZERODDS_POLICY_NT(DurabilityService, int32_t, max_instances)
    ZERODDS_POLICY_NT(DurabilityService, int32_t, max_samples_per_instance)

private:
    dds::core::Duration service_cleanup_delay_;
    HistoryKind::type history_kind_;
    int32_t history_depth_;
    int32_t max_samples_;
    int32_t max_instances_;
    int32_t max_samples_per_instance_;
};

/// DDS-XTypes 1.3 §7.6.3.1, Annex D `DataRepresentationQosPolicy`. `is_set()` tells whether the
/// application gave a list: a policy that was not given leaves the choice of the representation to
/// the Service (the C API's `is_set` = 0), one that was given is the list the entity uses.
class DataRepresentation {
public:
    // XCDR1, the default of DDS-XTypes 1.3 §7.6.3.1
    DataRepresentation()
        : value_(1, ::dds::core::policy::DataRepresentationId(0)), set_(false) {}
    explicit DataRepresentation(const ::dds::core::policy::DataRepresentationIdSeq& value)
        : value_(value), set_(true) {}
#if ZERODDS_CXX11
    DataRepresentation(const DataRepresentation& other) = default;
    DataRepresentation(DataRepresentation&&) = default;
    DataRepresentation& operator=(const DataRepresentation& other) = default;
    DataRepresentation& operator=(DataRepresentation&&) = default;
#endif

    DataRepresentation& value(const ::dds::core::policy::DataRepresentationIdSeq& v) {
        value_ = v;
        set_ = true;
        return *this;
    }
    const ::dds::core::policy::DataRepresentationIdSeq& value() const { return value_; }
    /// A reference the application may change: the policy counts as given.
    ::dds::core::policy::DataRepresentationIdSeq& value() {
        set_ = true;
        return value_;
    }
    /// Whether the application gave a list.
    bool is_set() const { return set_; }

private:
    ::dds::core::policy::DataRepresentationIdSeq value_;
    bool set_;
};

/// DDS-XTypes 1.3 §7.6.3.4, Annex D `TypeConsistencyEnforcementQosPolicy`: the kind and the five
/// flags. The kind has the four values of the PSM; the specification of XTypes 1.3 has two
/// (`DISALLOW_TYPE_COERCION`, `ALLOW_TYPE_COERCION`), and `ASSIGNABLE_TYPE_CONSISTENCY` is the
/// second, the three others the first. The defaults of the flags are those of §7.6.3.4.1.
class TypeConsistencyEnforcement {
public:
    TypeConsistencyEnforcement()
        : kind_(TypeConsistencyEnforcementKind::ASSIGNABLE_TYPE_CONSISTENCY),
          ignore_sequence_bounds_(true),
          ignore_string_bounds_(true),
          ignore_member_names_(false),
          prevent_type_widening_(false),
          force_type_validation_(false),
          set_(false) {}
    explicit TypeConsistencyEnforcement(TypeConsistencyEnforcementKind::type kind)
        : kind_(kind),
          ignore_sequence_bounds_(true),
          ignore_string_bounds_(true),
          ignore_member_names_(false),
          prevent_type_widening_(false),
          force_type_validation_(false),
          set_(true) {}
#if ZERODDS_CXX11
    TypeConsistencyEnforcement(const TypeConsistencyEnforcement& other) = default;
    TypeConsistencyEnforcement(TypeConsistencyEnforcement&&) = default;
    TypeConsistencyEnforcement& operator=(const TypeConsistencyEnforcement& other) = default;
    TypeConsistencyEnforcement& operator=(TypeConsistencyEnforcement&&) = default;
#endif

    TypeConsistencyEnforcementKind::type kind() const { return kind_; }
    TypeConsistencyEnforcementKind::type& kind() {
        set_ = true;
        return kind_;
    }
    TypeConsistencyEnforcement& kind(TypeConsistencyEnforcementKind::type value) {
        kind_ = value;
        set_ = true;
        return *this;
    }

    bool ignore_sequence_bounds() const { return ignore_sequence_bounds_; }
    TypeConsistencyEnforcement& ignore_sequence_bounds(bool value) {
        ignore_sequence_bounds_ = value;
        set_ = true;
        return *this;
    }
    bool ignore_string_bounds() const { return ignore_string_bounds_; }
    TypeConsistencyEnforcement& ignore_string_bounds(bool value) {
        ignore_string_bounds_ = value;
        set_ = true;
        return *this;
    }
    bool ignore_member_names() const { return ignore_member_names_; }
    TypeConsistencyEnforcement& ignore_member_names(bool value) {
        ignore_member_names_ = value;
        set_ = true;
        return *this;
    }
    bool prevent_type_widening() const { return prevent_type_widening_; }
    TypeConsistencyEnforcement& prevent_type_widening(bool value) {
        prevent_type_widening_ = value;
        set_ = true;
        return *this;
    }
    bool force_type_validation() const { return force_type_validation_; }
    TypeConsistencyEnforcement& force_type_validation(bool value) {
        force_type_validation_ = value;
        set_ = true;
        return *this;
    }

    /// Whether the application gave the policy.
    bool is_set() const { return set_; }

private:
    TypeConsistencyEnforcementKind::type kind_;
    bool ignore_sequence_bounds_;
    bool ignore_string_bounds_;
    bool ignore_member_names_;
    bool prevent_type_widening_;
    bool force_type_validation_;
    bool set_;
};

} // namespace detail

typedef TUserData<detail::UserData> UserData;
typedef TGroupData<detail::GroupData> GroupData;
typedef TTopicData<detail::TopicData> TopicData;
typedef TEntityFactory<detail::EntityFactory> EntityFactory;
typedef TTransportPriority<detail::TransportPriority> TransportPriority;
typedef TLifespan<detail::Lifespan> Lifespan;
typedef TDeadline<detail::Deadline> Deadline;
typedef TLatencyBudget<detail::LatencyBudget> LatencyBudget;
typedef TTimeBasedFilter<detail::TimeBasedFilter> TimeBasedFilter;
typedef TPartition<detail::Partition> Partition;
typedef TOwnership<detail::Ownership> Ownership;
typedef TOwnershipStrength<detail::OwnershipStrength> OwnershipStrength;
typedef TWriterDataLifecycle<detail::WriterDataLifecycle> WriterDataLifecycle;
typedef TReaderDataLifecycle<detail::ReaderDataLifecycle> ReaderDataLifecycle;
typedef TDurability<detail::Durability> Durability;
typedef TPresentation<detail::Presentation> Presentation;
typedef TReliability<detail::Reliability> Reliability;
typedef TDestinationOrder<detail::DestinationOrder> DestinationOrder;
typedef THistory<detail::History> History;
typedef TResourceLimits<detail::ResourceLimits> ResourceLimits;
typedef TLiveliness<detail::Liveliness> Liveliness;
typedef TDurabilityService<detail::DurabilityService> DurabilityService;
typedef TDataRepresentation<detail::DataRepresentation> DataRepresentation;
typedef TTypeConsistencyEnforcement<detail::TypeConsistencyEnforcement> TypeConsistencyEnforcement;

// ---- Policy ids and names (DDS 1.4 §2.2.3, Table of QosPolicyId_t) ----

#define ZERODDS_POLICY_TRAITS(POLICY, ID, NAME)                                    \
    template <>                                                                    \
    class policy_id<POLICY> {                                                      \
    public:                                                                        \
        static const dds::core::policy::QosPolicyId value = ID;                    \
    };                                                                             \
    template <>                                                                    \
    class policy_name<POLICY> {                                                    \
    public:                                                                        \
        static const std::string& name() {                                         \
            static const std::string the_name = NAME;                              \
            return the_name;                                                       \
        }                                                                          \
    };

ZERODDS_POLICY_TRAITS(UserData, 1, "UserData")
ZERODDS_POLICY_TRAITS(Durability, 2, "Durability")
ZERODDS_POLICY_TRAITS(Presentation, 3, "Presentation")
ZERODDS_POLICY_TRAITS(Deadline, 4, "Deadline")
ZERODDS_POLICY_TRAITS(LatencyBudget, 5, "LatencyBudget")
ZERODDS_POLICY_TRAITS(Ownership, 6, "Ownership")
ZERODDS_POLICY_TRAITS(OwnershipStrength, 7, "OwnershipStrength")
ZERODDS_POLICY_TRAITS(Liveliness, 8, "Liveliness")
ZERODDS_POLICY_TRAITS(TimeBasedFilter, 9, "TimeBasedFilter")
ZERODDS_POLICY_TRAITS(Partition, 10, "Partition")
ZERODDS_POLICY_TRAITS(Reliability, 11, "Reliability")
ZERODDS_POLICY_TRAITS(DestinationOrder, 12, "DestinationOrder")
ZERODDS_POLICY_TRAITS(History, 13, "History")
ZERODDS_POLICY_TRAITS(ResourceLimits, 14, "ResourceLimits")
ZERODDS_POLICY_TRAITS(EntityFactory, 15, "EntityFactory")
ZERODDS_POLICY_TRAITS(WriterDataLifecycle, 16, "WriterDataLifecycle")
ZERODDS_POLICY_TRAITS(ReaderDataLifecycle, 17, "ReaderDataLifecycle")
ZERODDS_POLICY_TRAITS(TopicData, 18, "TopicData")
ZERODDS_POLICY_TRAITS(GroupData, 19, "GroupData")
ZERODDS_POLICY_TRAITS(TransportPriority, 20, "TransportPriority")
ZERODDS_POLICY_TRAITS(Lifespan, 21, "Lifespan")
ZERODDS_POLICY_TRAITS(DurabilityService, 22, "DurabilityService")
ZERODDS_POLICY_TRAITS(DataRepresentation, 23, "DataRepresentation")
ZERODDS_POLICY_TRAITS(TypeConsistencyEnforcement, 24, "TypeConsistencyEnforcement")
#undef ZERODDS_POLICY_TRAITS

// The macros of the normative header (dds/core/policy/CorePolicy.hpp) for declaring and defining
// the traits of a policy of one's own.
#define OMG_DDS_POLICY_TRAITS(POLICY, ID)                       \
    template <>                                                  \
    class policy_id<POLICY> {                                    \
    public:                                                      \
        static const dds::core::policy::QosPolicyId value = ID;  \
    };                                                           \
    template <>                                                  \
    class policy_name<POLICY> {                                  \
    public:                                                      \
        static const std::string& name();                        \
    };

#define OMG_DDS_DEFINE_POLICY_TRAITS(POLICY, NAME)                              \
    const std::string& dds::core::policy::policy_name<POLICY>::name() {          \
        static std::string the_name = #NAME;                                     \
        return the_name;                                                         \
    }

} // namespace policy
} // namespace core
} // namespace dds

#undef ZERODDS_POLICY_NT
#undef ZERODDS_POLICY_CT
#undef ZERODDS_POLICY_DEFAULTS

#endif // ZERODDS_DDS_CORE_POLICY_COREPOLICY_HPP
