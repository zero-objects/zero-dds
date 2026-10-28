// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/policy/TCorePolicy.hpp — DDS-PSM-Cxx 1.0 §7.6.1 the QoS policies as
// templates over a delegate `D`. Kinds are safe_enum values, every attribute has a
// getter, a reference getter and a chainable setter.

#ifndef ZERODDS_DDS_CORE_POLICY_TCOREPOLICY_HPP
#define ZERODDS_DDS_CORE_POLICY_TCOREPOLICY_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <string>

#include "dds/core/Duration.hpp"
#include "dds/core/Value.hpp"
#include "dds/core/policy/PolicyKind.hpp"
#include "dds/core/types.hpp"

// The value accessors UserData, GroupData and TopicData share (DDS-PSM-Cxx 1.0
// §7.6.1: each holds a sequence of octets); the setters return the policy.
// `value(dst)` copies the value to `dst` and returns it (the form of the OMG
// headers).
#define ZERODDS_OCTET_POLICY_VALUE(TNAME)                                     \
    TNAME& value(const ::dds::core::ByteSeq& seq) {                           \
        this->d_.value(seq);                                                  \
        return *this;                                                         \
    }                                                                         \
    template <typename OCTET_ITER>                                            \
    TNAME& value(OCTET_ITER begin, OCTET_ITER end) {                          \
        this->d_.value(begin, end);                                           \
        return *this;                                                         \
    }                                                                         \
    ::dds::core::ByteSeq value() const { return this->d_.value(); }           \
    ::dds::core::ByteSeq& value(::dds::core::ByteSeq& dst) const {            \
        dst = this->d_.value();                                               \
        return dst;                                                           \
    }                                                                         \
    ::dds::core::ByteSeq& value() { return this->d_.value(); }                \
    const uint8_t* begin() const { return this->d_.begin(); }                 \
    const uint8_t* end() const { return this->d_.end(); }

namespace dds {
namespace core {
namespace policy {

/// The `UserData` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TUserData : public ::dds::core::Value<D> {
public:
    TUserData() : ::dds::core::Value<D>() {}
    explicit TUserData(const ::dds::core::ByteSeq& seq) : ::dds::core::Value<D>(seq) {}
    TUserData(const uint8_t* value_begin, const uint8_t* value_end)
        : ::dds::core::Value<D>(value_begin, value_end) {}
    explicit TUserData(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TUserData(const TUserData& other) = default;
    TUserData(TUserData&&) = default;
    TUserData& operator=(const TUserData& other) = default;
    TUserData& operator=(TUserData&&) = default;
#endif

    ZERODDS_OCTET_POLICY_VALUE(TUserData)
};

/// The `GroupData` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TGroupData : public ::dds::core::Value<D> {
public:
    TGroupData() : ::dds::core::Value<D>() {}
    explicit TGroupData(const ::dds::core::ByteSeq& seq) : ::dds::core::Value<D>(seq) {}
    TGroupData(const uint8_t* value_begin, const uint8_t* value_end)
        : ::dds::core::Value<D>(value_begin, value_end) {}
    explicit TGroupData(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TGroupData(const TGroupData& other) = default;
    TGroupData(TGroupData&&) = default;
    TGroupData& operator=(const TGroupData& other) = default;
    TGroupData& operator=(TGroupData&&) = default;
#endif

    ZERODDS_OCTET_POLICY_VALUE(TGroupData)
};

/// The `TopicData` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TTopicData : public ::dds::core::Value<D> {
public:
    TTopicData() : ::dds::core::Value<D>() {}
    explicit TTopicData(const ::dds::core::ByteSeq& seq) : ::dds::core::Value<D>(seq) {}
    TTopicData(const uint8_t* value_begin, const uint8_t* value_end)
        : ::dds::core::Value<D>(value_begin, value_end) {}
    explicit TTopicData(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TTopicData(const TTopicData& other) = default;
    TTopicData(TTopicData&&) = default;
    TTopicData& operator=(const TTopicData& other) = default;
    TTopicData& operator=(TTopicData&&) = default;
#endif

    ZERODDS_OCTET_POLICY_VALUE(TTopicData)
};

/// The `EntityFactory` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TEntityFactory : public ::dds::core::Value<D> {
public:
    TEntityFactory() : ::dds::core::Value<D>() {}
    explicit TEntityFactory(bool the_auto_enable) : ::dds::core::Value<D>(the_auto_enable) {}
    explicit TEntityFactory(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TEntityFactory(const TEntityFactory& other) = default;
    TEntityFactory(TEntityFactory&&) = default;
    TEntityFactory& operator=(const TEntityFactory& other) = default;
    TEntityFactory& operator=(TEntityFactory&&) = default;
#endif

    bool autoenable_created_entities() const { return this->d_.autoenable_created_entities(); }
    bool& autoenable_created_entities() { return this->d_.autoenable_created_entities(); }
    TEntityFactory& autoenable_created_entities(bool value) {
        this->d_.autoenable_created_entities(value);
        return *this;
    }

    static TEntityFactory AutoEnable() { return TEntityFactory(D::AutoEnable()); }
    static TEntityFactory ManuallyEnable() { return TEntityFactory(D::ManuallyEnable()); }
};

/// The `TransportPriority` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TTransportPriority : public ::dds::core::Value<D> {
public:
    TTransportPriority() : ::dds::core::Value<D>() {}
    explicit TTransportPriority(int32_t prio) : ::dds::core::Value<D>(prio) {}
    explicit TTransportPriority(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TTransportPriority(const TTransportPriority& other) = default;
    TTransportPriority(TTransportPriority&&) = default;
    TTransportPriority& operator=(const TTransportPriority& other) = default;
    TTransportPriority& operator=(TTransportPriority&&) = default;
#endif

    int32_t value() const { return this->d_.value(); }
    int32_t& value() { return this->d_.value(); }
    TTransportPriority& value(int32_t value) {
        this->d_.value(value);
        return *this;
    }
};

/// The `Lifespan` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TLifespan : public ::dds::core::Value<D> {
public:
    TLifespan() : ::dds::core::Value<D>() {}
    explicit TLifespan(const ::dds::core::Duration& d) : ::dds::core::Value<D>(d) {}
    explicit TLifespan(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TLifespan(const TLifespan& other) = default;
    TLifespan(TLifespan&&) = default;
    TLifespan& operator=(const TLifespan& other) = default;
    TLifespan& operator=(TLifespan&&) = default;
#endif

    ::dds::core::Duration duration() const { return this->d_.duration(); }
    ::dds::core::Duration& duration() { return this->d_.duration(); }
    TLifespan& duration(const ::dds::core::Duration& value) {
        this->d_.duration(value);
        return *this;
    }
};

/// The `Deadline` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TDeadline : public ::dds::core::Value<D> {
public:
    TDeadline() : ::dds::core::Value<D>() {}
    explicit TDeadline(const ::dds::core::Duration& d) : ::dds::core::Value<D>(d) {}
    explicit TDeadline(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TDeadline(const TDeadline& other) = default;
    TDeadline(TDeadline&&) = default;
    TDeadline& operator=(const TDeadline& other) = default;
    TDeadline& operator=(TDeadline&&) = default;
#endif

    ::dds::core::Duration period() const { return this->d_.period(); }
    ::dds::core::Duration& period() { return this->d_.period(); }
    TDeadline& period(const ::dds::core::Duration& value) {
        this->d_.period(value);
        return *this;
    }
};

/// The `LatencyBudget` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TLatencyBudget : public ::dds::core::Value<D> {
public:
    TLatencyBudget() : ::dds::core::Value<D>() {}
    explicit TLatencyBudget(const ::dds::core::Duration& d) : ::dds::core::Value<D>(d) {}
    explicit TLatencyBudget(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TLatencyBudget(const TLatencyBudget& other) = default;
    TLatencyBudget(TLatencyBudget&&) = default;
    TLatencyBudget& operator=(const TLatencyBudget& other) = default;
    TLatencyBudget& operator=(TLatencyBudget&&) = default;
#endif

    ::dds::core::Duration duration() const { return this->d_.duration(); }
    ::dds::core::Duration& duration() { return this->d_.duration(); }
    TLatencyBudget& duration(const ::dds::core::Duration& value) {
        this->d_.duration(value);
        return *this;
    }
};

/// The `TimeBasedFilter` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TTimeBasedFilter : public ::dds::core::Value<D> {
public:
    TTimeBasedFilter() : ::dds::core::Value<D>() {}
    explicit TTimeBasedFilter(const ::dds::core::Duration& d) : ::dds::core::Value<D>(d) {}
    explicit TTimeBasedFilter(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TTimeBasedFilter(const TTimeBasedFilter& other) = default;
    TTimeBasedFilter(TTimeBasedFilter&&) = default;
    TTimeBasedFilter& operator=(const TTimeBasedFilter& other) = default;
    TTimeBasedFilter& operator=(TTimeBasedFilter&&) = default;
#endif

    ::dds::core::Duration minimum_separation() const { return this->d_.minimum_separation(); }
    ::dds::core::Duration& minimum_separation() { return this->d_.minimum_separation(); }
    TTimeBasedFilter& minimum_separation(const ::dds::core::Duration& value) {
        this->d_.minimum_separation(value);
        return *this;
    }
};

/// The `Partition` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TPartition : public ::dds::core::Value<D> {
public:
    TPartition() : ::dds::core::Value<D>() {}
    explicit TPartition(const std::string& partition) : ::dds::core::Value<D>(partition) {}
    explicit TPartition(const ::dds::core::StringSeq& partitions) : ::dds::core::Value<D>(partitions) {}
    explicit TPartition(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TPartition(const TPartition& other) = default;
    TPartition(TPartition&&) = default;
    TPartition& operator=(const TPartition& other) = default;
    TPartition& operator=(TPartition&&) = default;
#endif

    TPartition& name(const ::dds::core::StringSeq& partitions) {
        this->d_.name(partitions);
        return *this;
    }
    ::dds::core::StringSeq name() const { return this->d_.name(); }
    /// Copies the names to `dst` and returns it (the form of the OMG headers).
    ::dds::core::StringSeq& name(::dds::core::StringSeq& dst) const {
        dst = this->d_.name();
        return dst;
    }
    ::dds::core::StringSeq& name() { return this->d_.name(); }
};

/// The `Ownership` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TOwnership : public ::dds::core::Value<D> {
public:
    TOwnership() : ::dds::core::Value<D>() {}
    explicit TOwnership(OwnershipKind::type the_kind) : ::dds::core::Value<D>(the_kind) {}
    explicit TOwnership(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TOwnership(const TOwnership& other) = default;
    TOwnership(TOwnership&&) = default;
    TOwnership& operator=(const TOwnership& other) = default;
    TOwnership& operator=(TOwnership&&) = default;
#endif

    OwnershipKind::type kind() const { return this->d_.kind(); }
    OwnershipKind::type& kind() { return this->d_.kind(); }
    TOwnership& kind(OwnershipKind::type value) {
        this->d_.kind(value);
        return *this;
    }

    static TOwnership Exclusive() { return TOwnership(D::Exclusive()); }
    static TOwnership Shared() { return TOwnership(D::Shared()); }
};

/// The `OwnershipStrength` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TOwnershipStrength : public ::dds::core::Value<D> {
public:
    TOwnershipStrength() : ::dds::core::Value<D>() {}
    explicit TOwnershipStrength(int32_t s) : ::dds::core::Value<D>(s) {}
    explicit TOwnershipStrength(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TOwnershipStrength(const TOwnershipStrength& other) = default;
    TOwnershipStrength(TOwnershipStrength&&) = default;
    TOwnershipStrength& operator=(const TOwnershipStrength& other) = default;
    TOwnershipStrength& operator=(TOwnershipStrength&&) = default;
#endif

    int32_t value() const { return this->d_.value(); }
    int32_t& value() { return this->d_.value(); }
    TOwnershipStrength& value(int32_t value) {
        this->d_.value(value);
        return *this;
    }
};

/// The `WriterDataLifecycle` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TWriterDataLifecycle : public ::dds::core::Value<D> {
public:
    TWriterDataLifecycle() : ::dds::core::Value<D>() {}
    explicit TWriterDataLifecycle(bool the_autodispose) : ::dds::core::Value<D>(the_autodispose) {}
    explicit TWriterDataLifecycle(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TWriterDataLifecycle(const TWriterDataLifecycle& other) = default;
    TWriterDataLifecycle(TWriterDataLifecycle&&) = default;
    TWriterDataLifecycle& operator=(const TWriterDataLifecycle& other) = default;
    TWriterDataLifecycle& operator=(TWriterDataLifecycle&&) = default;
#endif

    bool autodispose_unregistered_instances() const { return this->d_.autodispose_unregistered_instances(); }
    bool& autodispose_unregistered_instances() { return this->d_.autodispose_unregistered_instances(); }
    TWriterDataLifecycle& autodispose_unregistered_instances(bool value) {
        this->d_.autodispose_unregistered_instances(value);
        return *this;
    }

    static TWriterDataLifecycle AutoDisposeUnregisteredInstances() {
        return TWriterDataLifecycle(D::AutoDisposeUnregisteredInstances());
    }
    static TWriterDataLifecycle ManuallyDisposeUnregisteredInstances() {
        return TWriterDataLifecycle(D::ManuallyDisposeUnregisteredInstances());
    }
};

/// The `ReaderDataLifecycle` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TReaderDataLifecycle : public ::dds::core::Value<D> {
public:
    TReaderDataLifecycle() : ::dds::core::Value<D>() {}
    TReaderDataLifecycle(const ::dds::core::Duration& the_nowriter_delay, const ::dds::core::Duration& the_disposed_samples_delay) : ::dds::core::Value<D>(the_nowriter_delay, the_disposed_samples_delay) {}
    explicit TReaderDataLifecycle(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TReaderDataLifecycle(const TReaderDataLifecycle& other) = default;
    TReaderDataLifecycle(TReaderDataLifecycle&&) = default;
    TReaderDataLifecycle& operator=(const TReaderDataLifecycle& other) = default;
    TReaderDataLifecycle& operator=(TReaderDataLifecycle&&) = default;
#endif

    ::dds::core::Duration autopurge_nowriter_samples_delay() const { return this->d_.autopurge_nowriter_samples_delay(); }
    ::dds::core::Duration& autopurge_nowriter_samples_delay() { return this->d_.autopurge_nowriter_samples_delay(); }
    TReaderDataLifecycle& autopurge_nowriter_samples_delay(const ::dds::core::Duration& value) {
        this->d_.autopurge_nowriter_samples_delay(value);
        return *this;
    }
    ::dds::core::Duration autopurge_disposed_samples_delay() const { return this->d_.autopurge_disposed_samples_delay(); }
    ::dds::core::Duration& autopurge_disposed_samples_delay() { return this->d_.autopurge_disposed_samples_delay(); }
    TReaderDataLifecycle& autopurge_disposed_samples_delay(const ::dds::core::Duration& value) {
        this->d_.autopurge_disposed_samples_delay(value);
        return *this;
    }

    static TReaderDataLifecycle NoAutoPurgeDisposedSamples() {
        return TReaderDataLifecycle(D::NoAutoPurgeDisposedSamples());
    }
    static TReaderDataLifecycle AutoPurgeDisposedSamples(const ::dds::core::Duration& d) {
        return TReaderDataLifecycle(D::AutoPurgeDisposedSamples(d));
    }
};

/// The `Durability` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TDurability : public ::dds::core::Value<D> {
public:
    TDurability() : ::dds::core::Value<D>() {}
    explicit TDurability(DurabilityKind::type the_kind) : ::dds::core::Value<D>(the_kind) {}
    explicit TDurability(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TDurability(const TDurability& other) = default;
    TDurability(TDurability&&) = default;
    TDurability& operator=(const TDurability& other) = default;
    TDurability& operator=(TDurability&&) = default;
#endif

    DurabilityKind::type kind() const { return this->d_.kind(); }
    DurabilityKind::type& kind() { return this->d_.kind(); }
    TDurability& kind(DurabilityKind::type value) {
        this->d_.kind(value);
        return *this;
    }

    static TDurability Volatile() { return TDurability(D::Volatile()); }
    static TDurability TransientLocal() { return TDurability(D::TransientLocal()); }
    static TDurability Transient() { return TDurability(D::Transient()); }
    static TDurability Persistent() { return TDurability(D::Persistent()); }
};

/// The `Presentation` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TPresentation : public ::dds::core::Value<D> {
public:
    TPresentation() : ::dds::core::Value<D>() {}
    TPresentation(PresentationAccessScopeKind::type the_access_scope, bool the_coherent_access, bool the_ordered_access) : ::dds::core::Value<D>(the_access_scope, the_coherent_access, the_ordered_access) {}
    explicit TPresentation(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TPresentation(const TPresentation& other) = default;
    TPresentation(TPresentation&&) = default;
    TPresentation& operator=(const TPresentation& other) = default;
    TPresentation& operator=(TPresentation&&) = default;
#endif

    PresentationAccessScopeKind::type access_scope() const { return this->d_.access_scope(); }
    PresentationAccessScopeKind::type& access_scope() { return this->d_.access_scope(); }
    TPresentation& access_scope(PresentationAccessScopeKind::type value) {
        this->d_.access_scope(value);
        return *this;
    }
    bool coherent_access() const { return this->d_.coherent_access(); }
    bool& coherent_access() { return this->d_.coherent_access(); }
    TPresentation& coherent_access(bool value) {
        this->d_.coherent_access(value);
        return *this;
    }
    bool ordered_access() const { return this->d_.ordered_access(); }
    bool& ordered_access() { return this->d_.ordered_access(); }
    TPresentation& ordered_access(bool value) {
        this->d_.ordered_access(value);
        return *this;
    }

    static TPresentation GroupAccessScope(bool coherent = false, bool ordered = false) {
        return TPresentation(D::GroupAccessScope(coherent, ordered));
    }
    static TPresentation InstanceAccessScope(bool coherent = false, bool ordered = false) {
        return TPresentation(D::InstanceAccessScope(coherent, ordered));
    }
    static TPresentation TopicAccessScope(bool coherent = false, bool ordered = false) {
        return TPresentation(D::TopicAccessScope(coherent, ordered));
    }
};

/// The `Reliability` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TReliability : public ::dds::core::Value<D> {
public:
    TReliability() : ::dds::core::Value<D>() {}
    TReliability(ReliabilityKind::type the_kind, const ::dds::core::Duration& the_max_blocking_time) : ::dds::core::Value<D>(the_kind, the_max_blocking_time) {}
    explicit TReliability(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TReliability(const TReliability& other) = default;
    TReliability(TReliability&&) = default;
    TReliability& operator=(const TReliability& other) = default;
    TReliability& operator=(TReliability&&) = default;
#endif

    ReliabilityKind::type kind() const { return this->d_.kind(); }
    ReliabilityKind::type& kind() { return this->d_.kind(); }
    TReliability& kind(ReliabilityKind::type value) {
        this->d_.kind(value);
        return *this;
    }
    ::dds::core::Duration max_blocking_time() const { return this->d_.max_blocking_time(); }
    ::dds::core::Duration& max_blocking_time() { return this->d_.max_blocking_time(); }
    TReliability& max_blocking_time(const ::dds::core::Duration& value) {
        this->d_.max_blocking_time(value);
        return *this;
    }

    static TReliability Reliable(const ::dds::core::Duration& d = ::dds::core::Duration::infinite()) {
        return TReliability(D::Reliable(d));
    }
    static TReliability BestEffort() { return TReliability(D::BestEffort()); }
};

/// The `DestinationOrder` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TDestinationOrder : public ::dds::core::Value<D> {
public:
    TDestinationOrder() : ::dds::core::Value<D>() {}
    explicit TDestinationOrder(DestinationOrderKind::type the_kind) : ::dds::core::Value<D>(the_kind) {}
    explicit TDestinationOrder(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TDestinationOrder(const TDestinationOrder& other) = default;
    TDestinationOrder(TDestinationOrder&&) = default;
    TDestinationOrder& operator=(const TDestinationOrder& other) = default;
    TDestinationOrder& operator=(TDestinationOrder&&) = default;
#endif

    DestinationOrderKind::type kind() const { return this->d_.kind(); }
    DestinationOrderKind::type& kind() { return this->d_.kind(); }
    TDestinationOrder& kind(DestinationOrderKind::type value) {
        this->d_.kind(value);
        return *this;
    }

    static TDestinationOrder SourceTimestamp() { return TDestinationOrder(D::SourceTimestamp()); }
    static TDestinationOrder ReceptionTimestamp() { return TDestinationOrder(D::ReceptionTimestamp()); }
};

/// The `History` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class THistory : public ::dds::core::Value<D> {
public:
    THistory() : ::dds::core::Value<D>() {}
    THistory(HistoryKind::type the_kind, int32_t the_depth) : ::dds::core::Value<D>(the_kind, the_depth) {}
    explicit THistory(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    THistory(const THistory& other) = default;
    THistory(THistory&&) = default;
    THistory& operator=(const THistory& other) = default;
    THistory& operator=(THistory&&) = default;
#endif

    HistoryKind::type kind() const { return this->d_.kind(); }
    HistoryKind::type& kind() { return this->d_.kind(); }
    THistory& kind(HistoryKind::type value) {
        this->d_.kind(value);
        return *this;
    }
    int32_t depth() const { return this->d_.depth(); }
    int32_t& depth() { return this->d_.depth(); }
    THistory& depth(int32_t value) {
        this->d_.depth(value);
        return *this;
    }

    static THistory KeepAll() { return THistory(D::KeepAll()); }
    static THistory KeepLast(uint32_t depth) { return THistory(D::KeepLast(depth)); }
};

/// The `ResourceLimits` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TResourceLimits : public ::dds::core::Value<D> {
public:
    TResourceLimits() : ::dds::core::Value<D>() {}
    TResourceLimits(int32_t the_max_samples, int32_t the_max_instances, int32_t the_max_samples_per_instance) : ::dds::core::Value<D>(the_max_samples, the_max_instances, the_max_samples_per_instance) {}
    explicit TResourceLimits(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TResourceLimits(const TResourceLimits& other) = default;
    TResourceLimits(TResourceLimits&&) = default;
    TResourceLimits& operator=(const TResourceLimits& other) = default;
    TResourceLimits& operator=(TResourceLimits&&) = default;
#endif

    int32_t max_samples() const { return this->d_.max_samples(); }
    int32_t& max_samples() { return this->d_.max_samples(); }
    TResourceLimits& max_samples(int32_t value) {
        this->d_.max_samples(value);
        return *this;
    }
    int32_t max_instances() const { return this->d_.max_instances(); }
    int32_t& max_instances() { return this->d_.max_instances(); }
    TResourceLimits& max_instances(int32_t value) {
        this->d_.max_instances(value);
        return *this;
    }
    int32_t max_samples_per_instance() const { return this->d_.max_samples_per_instance(); }
    int32_t& max_samples_per_instance() { return this->d_.max_samples_per_instance(); }
    TResourceLimits& max_samples_per_instance(int32_t value) {
        this->d_.max_samples_per_instance(value);
        return *this;
    }
};

/// The `Liveliness` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TLiveliness : public ::dds::core::Value<D> {
public:
    TLiveliness() : ::dds::core::Value<D>() {}
    TLiveliness(LivelinessKind::type the_kind, const ::dds::core::Duration& the_lease_duration) : ::dds::core::Value<D>(the_kind, the_lease_duration) {}
    explicit TLiveliness(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TLiveliness(const TLiveliness& other) = default;
    TLiveliness(TLiveliness&&) = default;
    TLiveliness& operator=(const TLiveliness& other) = default;
    TLiveliness& operator=(TLiveliness&&) = default;
#endif

    LivelinessKind::type kind() const { return this->d_.kind(); }
    LivelinessKind::type& kind() { return this->d_.kind(); }
    TLiveliness& kind(LivelinessKind::type value) {
        this->d_.kind(value);
        return *this;
    }
    ::dds::core::Duration lease_duration() const { return this->d_.lease_duration(); }
    ::dds::core::Duration& lease_duration() { return this->d_.lease_duration(); }
    TLiveliness& lease_duration(const ::dds::core::Duration& value) {
        this->d_.lease_duration(value);
        return *this;
    }

    static TLiveliness Automatic() { return TLiveliness(D::Automatic()); }
    static TLiveliness ManualByParticipant(
        const ::dds::core::Duration& lease = ::dds::core::Duration::infinite()) {
        return TLiveliness(D::ManualByParticipant(lease));
    }
    static TLiveliness ManualByTopic(
        const ::dds::core::Duration& lease = ::dds::core::Duration::infinite()) {
        return TLiveliness(D::ManualByTopic(lease));
    }
};

/// The `DurabilityService` policy over any delegate `D` (DDS-PSM-Cxx 1.0 §7.6.1).
template <typename D>
class TDurabilityService : public ::dds::core::Value<D> {
public:
    TDurabilityService() : ::dds::core::Value<D>() {}
    TDurabilityService(const ::dds::core::Duration& the_service_cleanup_delay, HistoryKind::type the_history_kind, int32_t the_history_depth, int32_t the_max_samples, int32_t the_max_instances, int32_t the_max_samples_per_instance) : ::dds::core::Value<D>(the_service_cleanup_delay, the_history_kind, the_history_depth, the_max_samples, the_max_instances, the_max_samples_per_instance) {}
    explicit TDurabilityService(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TDurabilityService(const TDurabilityService& other) = default;
    TDurabilityService(TDurabilityService&&) = default;
    TDurabilityService& operator=(const TDurabilityService& other) = default;
    TDurabilityService& operator=(TDurabilityService&&) = default;
#endif

    ::dds::core::Duration service_cleanup_delay() const { return this->d_.service_cleanup_delay(); }
    ::dds::core::Duration& service_cleanup_delay() { return this->d_.service_cleanup_delay(); }
    TDurabilityService& service_cleanup_delay(const ::dds::core::Duration& value) {
        this->d_.service_cleanup_delay(value);
        return *this;
    }
    HistoryKind::type history_kind() const { return this->d_.history_kind(); }
    HistoryKind::type& history_kind() { return this->d_.history_kind(); }
    TDurabilityService& history_kind(HistoryKind::type value) {
        this->d_.history_kind(value);
        return *this;
    }
    int32_t history_depth() const { return this->d_.history_depth(); }
    int32_t& history_depth() { return this->d_.history_depth(); }
    TDurabilityService& history_depth(int32_t value) {
        this->d_.history_depth(value);
        return *this;
    }
    int32_t max_samples() const { return this->d_.max_samples(); }
    int32_t& max_samples() { return this->d_.max_samples(); }
    TDurabilityService& max_samples(int32_t value) {
        this->d_.max_samples(value);
        return *this;
    }
    int32_t max_instances() const { return this->d_.max_instances(); }
    int32_t& max_instances() { return this->d_.max_instances(); }
    TDurabilityService& max_instances(int32_t value) {
        this->d_.max_instances(value);
        return *this;
    }
    int32_t max_samples_per_instance() const { return this->d_.max_samples_per_instance(); }
    int32_t& max_samples_per_instance() { return this->d_.max_samples_per_instance(); }
    TDurabilityService& max_samples_per_instance(int32_t value) {
        this->d_.max_samples_per_instance(value);
        return *this;
    }
};

/// The `DataRepresentation` policy over any delegate `D` (DDS-PSM-Cxx 1.0, dds/core/policy/TCorePolicy.hpp,
/// DDS-XTypes 1.3 §7.6.3.1).
template <typename D>
class TDataRepresentation : public ::dds::core::Value<D> {
public:
    TDataRepresentation() : ::dds::core::Value<D>() {}
    explicit TDataRepresentation(const ::dds::core::policy::DataRepresentationIdSeq& value)
        : ::dds::core::Value<D>(value) {}
    explicit TDataRepresentation(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TDataRepresentation(const TDataRepresentation& other) = default;
    TDataRepresentation(TDataRepresentation&&) = default;
    TDataRepresentation& operator=(const TDataRepresentation& other) = default;
    TDataRepresentation& operator=(TDataRepresentation&&) = default;
#endif

    TDataRepresentation& value(const ::dds::core::policy::DataRepresentationIdSeq& value) {
        this->d_.value(value);
        return *this;
    }
    const ::dds::core::policy::DataRepresentationIdSeq value() const { return this->d_.value(); }
    ::dds::core::policy::DataRepresentationIdSeq& value(
        ::dds::core::policy::DataRepresentationIdSeq& dst) const {
        dst = this->d_.value();
        return dst;
    }
    /// Whether the application gave a list (DDS-XTypes 1.3 §7.6.3.1: an entity without one lets
    /// the Service choose).
    bool is_set() const { return this->d_.is_set(); }
};

/// The `TypeConsistencyEnforcement` policy over any delegate `D` (DDS-XTypes 1.3 §7.6.3.4).
template <typename D>
class TTypeConsistencyEnforcement : public ::dds::core::Value<D> {
public:
    TTypeConsistencyEnforcement() : ::dds::core::Value<D>() {}
    explicit TTypeConsistencyEnforcement(TypeConsistencyEnforcementKind::type kind)
        : ::dds::core::Value<D>(kind) {}
    explicit TTypeConsistencyEnforcement(const D& d) : ::dds::core::Value<D>(d) {}
#if ZERODDS_CXX11
    TTypeConsistencyEnforcement(const TTypeConsistencyEnforcement& other) = default;
    TTypeConsistencyEnforcement(TTypeConsistencyEnforcement&&) = default;
    TTypeConsistencyEnforcement& operator=(const TTypeConsistencyEnforcement& other) = default;
    TTypeConsistencyEnforcement& operator=(TTypeConsistencyEnforcement&&) = default;
#endif

    TTypeConsistencyEnforcement& kind(TypeConsistencyEnforcementKind::type value) {
        this->d_.kind(value);
        return *this;
    }
    TypeConsistencyEnforcementKind::type kind() const { return this->d_.kind(); }

    // The five flags of DDS-XTypes 1.3 Annex D `TypeConsistencyEnforcementQosPolicy`.
    bool ignore_sequence_bounds() const { return this->d_.ignore_sequence_bounds(); }
    TTypeConsistencyEnforcement& ignore_sequence_bounds(bool value) {
        this->d_.ignore_sequence_bounds(value);
        return *this;
    }
    bool ignore_string_bounds() const { return this->d_.ignore_string_bounds(); }
    TTypeConsistencyEnforcement& ignore_string_bounds(bool value) {
        this->d_.ignore_string_bounds(value);
        return *this;
    }
    bool ignore_member_names() const { return this->d_.ignore_member_names(); }
    TTypeConsistencyEnforcement& ignore_member_names(bool value) {
        this->d_.ignore_member_names(value);
        return *this;
    }
    bool prevent_type_widening() const { return this->d_.prevent_type_widening(); }
    TTypeConsistencyEnforcement& prevent_type_widening(bool value) {
        this->d_.prevent_type_widening(value);
        return *this;
    }
    bool force_type_validation() const { return this->d_.force_type_validation(); }
    TTypeConsistencyEnforcement& force_type_validation(bool value) {
        this->d_.force_type_validation(value);
        return *this;
    }
    /// Whether the application gave the policy.
    bool is_set() const { return this->d_.is_set(); }
};

} // namespace policy
} // namespace core
} // namespace dds

#undef ZERODDS_OCTET_POLICY_VALUE

#endif // ZERODDS_DDS_CORE_POLICY_TCOREPOLICY_HPP
