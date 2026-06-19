// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/policy/CorePolicy.hpp — DDS-PSM-Cxx 1.0 §7.5.6 all 22 QoS policies.

#ifndef ZERODDS_DDS_CORE_POLICY_COREPOLICY_HPP
#define ZERODDS_DDS_CORE_POLICY_COREPOLICY_HPP

#include <cstdint>
#include <string>
#include <vector>

#include "dds/core/Time.hpp"

namespace dds {
namespace core {
namespace policy {

/// ReliabilityKind (Spec §7.5.6.13).
enum class ReliabilityKind : uint32_t {
    BestEffort = 1,
    Reliable = 2,
};

/// DurabilityKind (Spec §7.5.6.4).
enum class DurabilityKind : uint32_t {
    Volatile = 0,
    TransientLocal = 1,
    Transient = 2,
    Persistent = 3,
};

/// HistoryKind (Spec §7.5.6.10).
enum class HistoryKind : uint32_t { KeepLast = 0, KeepAll = 1 };

/// OwnershipKind (Spec §7.5.6.11).
enum class OwnershipKind : uint32_t { Shared = 0, Exclusive = 1 };

/// LivelinessKind (Spec §7.5.6.9).
enum class LivelinessKind : uint32_t {
    Automatic = 0,
    ManualByParticipant = 1,
    ManualByTopic = 2,
};

/// DestinationOrderKind (Spec §7.5.6.5).
enum class DestinationOrderKind : uint32_t {
    ByReceptionTimestamp = 0,
    BySourceTimestamp = 1,
};

/// PresentationAccessScopeKind (Spec §7.5.6.12).
enum class PresentationAccessScopeKind : uint32_t {
    Instance = 0,
    Topic = 1,
    Group = 2,
};

// ---- Policy classes --------------------------------------------------

/// UserData (Spec §7.5.6.16).
class UserData {
public:
    UserData() = default;
    explicit UserData(std::vector<uint8_t> v) : value_(std::move(v)) {}
    const std::vector<uint8_t>& value() const { return value_; }
    void value(std::vector<uint8_t> v) { value_ = std::move(v); }
private:
    std::vector<uint8_t> value_;
};

/// TopicData (Spec §7.5.6.17).
class TopicData : public UserData {
public:
    using UserData::UserData;
};

/// GroupData (Spec §7.5.6.7).
class GroupData : public UserData {
public:
    using UserData::UserData;
};

/// Reliability (Spec §7.5.6.13).
class Reliability {
public:
    Reliability() = default;
    Reliability(ReliabilityKind k, Duration t) : kind_(k), max_blocking_time_(t) {}
    static Reliability Reliable_(Duration t = Duration::from_millis(100)) {
        return Reliability(ReliabilityKind::Reliable, t);
    }
    static Reliability BestEffort() {
        return Reliability(ReliabilityKind::BestEffort, Duration::from_millis(100));
    }
    ReliabilityKind kind() const { return kind_; }
    Duration max_blocking_time() const { return max_blocking_time_; }
    void kind(ReliabilityKind k) { kind_ = k; }
    void max_blocking_time(Duration t) { max_blocking_time_ = t; }
private:
    ReliabilityKind kind_{ReliabilityKind::Reliable};
    Duration max_blocking_time_{Duration::from_millis(100)};
};

/// Durability (Spec §7.5.6.4).
class Durability {
public:
    Durability() = default;
    explicit Durability(DurabilityKind k) : kind_(k) {}
    static Durability Volatile() { return Durability(DurabilityKind::Volatile); }
    static Durability TransientLocal() { return Durability(DurabilityKind::TransientLocal); }
    static Durability Transient() { return Durability(DurabilityKind::Transient); }
    static Durability Persistent() { return Durability(DurabilityKind::Persistent); }
    DurabilityKind kind() const { return kind_; }
    void kind(DurabilityKind k) { kind_ = k; }
private:
    DurabilityKind kind_{DurabilityKind::Volatile};
};

/// History (Spec §7.5.6.10).
class History {
public:
    History() = default;
    History(HistoryKind k, int32_t depth) : kind_(k), depth_(depth) {}
    static History KeepLast(int32_t depth = 1) { return History(HistoryKind::KeepLast, depth); }
    static History KeepAll() { return History(HistoryKind::KeepAll, -1); }
    HistoryKind kind() const { return kind_; }
    int32_t depth() const { return depth_; }
    void kind(HistoryKind k) { kind_ = k; }
    void depth(int32_t d) { depth_ = d; }
private:
    HistoryKind kind_{HistoryKind::KeepLast};
    int32_t depth_{1};
};

/// Deadline (Spec §7.5.6.2).
class Deadline {
public:
    Deadline() = default;
    explicit Deadline(Duration period) : period_(period) {}
    Duration period() const { return period_; }
    void period(Duration p) { period_ = p; }
private:
    Duration period_{Duration::infinite()};
};

/// LatencyBudget (Spec §7.5.6.8).
class LatencyBudget {
public:
    LatencyBudget() = default;
    explicit LatencyBudget(Duration d) : duration_(d) {}
    Duration duration() const { return duration_; }
    void duration(Duration d) { duration_ = d; }
private:
    Duration duration_{Duration::zero()};
};

/// Lifespan (Spec §7.5.6.20).
class Lifespan {
public:
    Lifespan() = default;
    explicit Lifespan(Duration d) : duration_(d) {}
    Duration duration() const { return duration_; }
    void duration(Duration d) { duration_ = d; }
private:
    Duration duration_{Duration::infinite()};
};

/// TimeBasedFilter (Spec §7.5.6.18).
class TimeBasedFilter {
public:
    TimeBasedFilter() = default;
    explicit TimeBasedFilter(Duration d) : minimum_separation_(d) {}
    Duration minimum_separation() const { return minimum_separation_; }
    void minimum_separation(Duration d) { minimum_separation_ = d; }
private:
    Duration minimum_separation_{Duration::zero()};
};

/// Liveliness (Spec §7.5.6.9).
class Liveliness {
public:
    Liveliness() = default;
    Liveliness(LivelinessKind k, Duration lease) : kind_(k), lease_duration_(lease) {}
    static Liveliness Automatic() {
        return Liveliness(LivelinessKind::Automatic, Duration::infinite());
    }
    LivelinessKind kind() const { return kind_; }
    Duration lease_duration() const { return lease_duration_; }
    void kind(LivelinessKind k) { kind_ = k; }
    void lease_duration(Duration d) { lease_duration_ = d; }
private:
    LivelinessKind kind_{LivelinessKind::Automatic};
    Duration lease_duration_{Duration::infinite()};
};

/// Ownership (Spec §7.5.6.11).
class Ownership {
public:
    Ownership() = default;
    explicit Ownership(OwnershipKind k) : kind_(k) {}
    static Ownership Shared() { return Ownership(OwnershipKind::Shared); }
    static Ownership Exclusive() { return Ownership(OwnershipKind::Exclusive); }
    OwnershipKind kind() const { return kind_; }
    void kind(OwnershipKind k) { kind_ = k; }
private:
    OwnershipKind kind_{OwnershipKind::Shared};
};

/// OwnershipStrength (Spec §7.5.6.21).
class OwnershipStrength {
public:
    OwnershipStrength() = default;
    explicit OwnershipStrength(int32_t v) : value_(v) {}
    int32_t value() const { return value_; }
    void value(int32_t v) { value_ = v; }
private:
    int32_t value_{0};
};

/// DestinationOrder (Spec §7.5.6.5).
class DestinationOrder {
public:
    DestinationOrder() = default;
    explicit DestinationOrder(DestinationOrderKind k) : kind_(k) {}
    DestinationOrderKind kind() const { return kind_; }
    void kind(DestinationOrderKind k) { kind_ = k; }
private:
    DestinationOrderKind kind_{DestinationOrderKind::ByReceptionTimestamp};
};

/// Presentation (Spec §7.5.6.12).
class Presentation {
public:
    Presentation() = default;
    Presentation(PresentationAccessScopeKind s, bool coherent, bool ordered)
        : access_scope_(s), coherent_(coherent), ordered_(ordered) {}
    PresentationAccessScopeKind access_scope() const { return access_scope_; }
    bool coherent_access() const { return coherent_; }
    bool ordered_access() const { return ordered_; }
    void access_scope(PresentationAccessScopeKind s) { access_scope_ = s; }
    void coherent_access(bool v) { coherent_ = v; }
    void ordered_access(bool v) { ordered_ = v; }
private:
    PresentationAccessScopeKind access_scope_{PresentationAccessScopeKind::Instance};
    bool coherent_{false};
    bool ordered_{false};
};

/// Partition (spec §7.5.6.0 — DDS-PSM-Cxx without a number; corresponds to DCPS §2.2.3.13).
class Partition {
public:
    Partition() = default;
    explicit Partition(std::vector<std::string> names) : names_(std::move(names)) {}
    const std::vector<std::string>& name() const { return names_; }
    void name(std::vector<std::string> names) { names_ = std::move(names); }
private:
    std::vector<std::string> names_;
};

/// ResourceLimits (Spec §7.5.6.15).
class ResourceLimits {
public:
    ResourceLimits() = default;
    ResourceLimits(int32_t s, int32_t i, int32_t spi)
        : max_samples_(s), max_instances_(i), max_samples_per_instance_(spi) {}
    int32_t max_samples() const { return max_samples_; }
    int32_t max_instances() const { return max_instances_; }
    int32_t max_samples_per_instance() const { return max_samples_per_instance_; }
    void max_samples(int32_t v) { max_samples_ = v; }
    void max_instances(int32_t v) { max_instances_ = v; }
    void max_samples_per_instance(int32_t v) { max_samples_per_instance_ = v; }
private:
    int32_t max_samples_{1000};
    int32_t max_instances_{10};
    int32_t max_samples_per_instance_{100};
};

/// TransportPriority (Spec §7.5.6.19).
class TransportPriority {
public:
    TransportPriority() = default;
    explicit TransportPriority(int32_t v) : value_(v) {}
    int32_t value() const { return value_; }
    void value(int32_t v) { value_ = v; }
private:
    int32_t value_{0};
};

/// EntityFactory (Spec §7.5.6.6).
class EntityFactory {
public:
    EntityFactory() = default;
    explicit EntityFactory(bool autoenable) : autoenable_(autoenable) {}
    static EntityFactory AutoEnable() { return EntityFactory(true); }
    static EntityFactory ManuallyEnable() { return EntityFactory(false); }
    bool autoenable_created_entities() const { return autoenable_; }
    void autoenable_created_entities(bool v) { autoenable_ = v; }
private:
    bool autoenable_{true};
};

/// WriterDataLifecycle (Spec §7.5.6.22).
class WriterDataLifecycle {
public:
    WriterDataLifecycle() = default;
    explicit WriterDataLifecycle(bool autodispose)
        : autodispose_unregistered_instances_(autodispose) {}
    bool autodispose_unregistered_instances() const {
        return autodispose_unregistered_instances_;
    }
    void autodispose_unregistered_instances(bool v) {
        autodispose_unregistered_instances_ = v;
    }
private:
    bool autodispose_unregistered_instances_{true};
};

/// ReaderDataLifecycle (Spec §7.5.6.14).
class ReaderDataLifecycle {
public:
    ReaderDataLifecycle() = default;
    ReaderDataLifecycle(Duration nowriter, Duration disposed)
        : autopurge_nowriter_samples_delay_(nowriter),
          autopurge_disposed_samples_delay_(disposed) {}
    Duration autopurge_nowriter_samples_delay() const { return autopurge_nowriter_samples_delay_; }
    Duration autopurge_disposed_samples_delay() const { return autopurge_disposed_samples_delay_; }
    void autopurge_nowriter_samples_delay(Duration d) { autopurge_nowriter_samples_delay_ = d; }
    void autopurge_disposed_samples_delay(Duration d) { autopurge_disposed_samples_delay_ = d; }
private:
    Duration autopurge_nowriter_samples_delay_{Duration::infinite()};
    Duration autopurge_disposed_samples_delay_{Duration::infinite()};
};

/// DurabilityService (Spec §7.5.6.3).
class DurabilityService {
public:
    DurabilityService() = default;
    DurabilityService(Duration cleanup, HistoryKind hk, int32_t hd, int32_t s, int32_t i,
                      int32_t spi)
        : service_cleanup_delay_(cleanup),
          history_kind_(hk),
          history_depth_(hd),
          max_samples_(s),
          max_instances_(i),
          max_samples_per_instance_(spi) {}
    Duration service_cleanup_delay() const { return service_cleanup_delay_; }
    HistoryKind history_kind() const { return history_kind_; }
    int32_t history_depth() const { return history_depth_; }
    int32_t max_samples() const { return max_samples_; }
    int32_t max_instances() const { return max_instances_; }
    int32_t max_samples_per_instance() const { return max_samples_per_instance_; }
    void service_cleanup_delay(Duration d) { service_cleanup_delay_ = d; }
    void history_kind(HistoryKind k) { history_kind_ = k; }
    void history_depth(int32_t v) { history_depth_ = v; }
    void max_samples(int32_t v) { max_samples_ = v; }
    void max_instances(int32_t v) { max_instances_ = v; }
    void max_samples_per_instance(int32_t v) { max_samples_per_instance_ = v; }
private:
    Duration service_cleanup_delay_{Duration::zero()};
    HistoryKind history_kind_{HistoryKind::KeepLast};
    int32_t history_depth_{1};
    int32_t max_samples_{-1};
    int32_t max_instances_{-1};
    int32_t max_samples_per_instance_{-1};
};

} // namespace policy
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_POLICY_COREPOLICY_HPP
