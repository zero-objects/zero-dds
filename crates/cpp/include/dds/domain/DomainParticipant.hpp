// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/domain/DomainParticipant.hpp — DDS-PSM-Cxx 1.0 §7.7.

#ifndef ZERODDS_DDS_DOMAIN_DOMAINPARTICIPANT_HPP
#define ZERODDS_DDS_DOMAIN_DOMAINPARTICIPANT_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include <stdint.h>
#include <algorithm>
#include <utility>
#include <vector>

#include "dds/core/Reference.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/core/TEntity.hpp"
#include "dds/domain/detail/DomainParticipantDelegate.hpp"
#include "dds/domain/DomainParticipantListener.hpp"
#include "dds/domain/TDomainParticipant.hpp"
#include "dds/topic/BuiltinTopic.hpp"
#include "dds/topic/AnyTopic.hpp"
#include "dds/topic/TTopicDescription.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {
class Subscriber;
} // namespace sub

namespace domain {
namespace detail {
struct ParticipantAccess;

/// The participants this process created and the application still holds.
/// `dds::domain::find` (DDS-PSM-Cxx 1.0 §7.7) looks one up by domain id;
/// the registry holds no strong reference, so it does not keep one alive.
class ParticipantRegistry {
    typedef std::pair<uint32_t, ::zerodds::cxx::weak_ptr<DomainParticipantDelegate> > Entry;
    typedef std::vector<Entry> Entries;

    /// Matches the entries whose participant is gone.
    struct IsExpired {
        bool operator()(const Entry& e) const { return e.second.expired(); }
    };

public:
    static void add(uint32_t domain_id, const ::zerodds::cxx::shared_ptr<DomainParticipantDelegate>& d) {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex());
        Entries& list = entries();
        list.erase(std::remove_if(list.begin(), list.end(), IsExpired()), list.end());
        list.push_back(Entry(domain_id, ::zerodds::cxx::weak_ptr<DomainParticipantDelegate>(d)));
    }
    /// A live participant of `domain_id`, or an empty pointer.
    static ::zerodds::cxx::shared_ptr<DomainParticipantDelegate> find(uint32_t domain_id) {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex());
        const Entries& list = entries();
        for (Entries::const_iterator it = list.begin(); it != list.end(); ++it) {
            if (it->first == domain_id) {
                ::zerodds::cxx::shared_ptr<DomainParticipantDelegate> d = it->second.lock();
                if (d) {
                    return d;
                }
            }
        }
        return ::zerodds::cxx::shared_ptr<DomainParticipantDelegate>();
    }

    /// Every participant of the process that is still alive.
    static std::vector< ::zerodds::cxx::shared_ptr<DomainParticipantDelegate> > all() {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex());
        std::vector< ::zerodds::cxx::shared_ptr<DomainParticipantDelegate> > out;
        const Entries& list = entries();
        for (Entries::const_iterator it = list.begin(); it != list.end(); ++it) {
            ::zerodds::cxx::shared_ptr<DomainParticipantDelegate> d = it->second.lock();
            if (d) {
                out.push_back(d);
            }
        }
        return out;
    }

private:
    static ::zerodds::cxx::mutex& mutex() {
        static ::zerodds::cxx::mutex m;
        return m;
    }
    static Entries& entries() {
        static Entries v;
        return v;
    }
};
}

/// DomainParticipant (Spec §7.7). Reference type with delegate pattern
/// (DDS-PSM-Cxx 1.0 §7.5.1.1, §7.5.1.2): `TDomainParticipant` over ZeroDDS's delegate.
class DomainParticipant : public TDomainParticipant<detail::DomainParticipantDelegate> {
    friend struct detail::ParticipantAccess;

public:
    typedef TDomainParticipant<detail::DomainParticipantDelegate> BASE_T;
    typedef detail::DomainParticipantDelegate DELEGATE_T;

    /// A null participant.
protected:
    DomainParticipant() ZERODDS_NOEXCEPT {}
public:

    /// A participant over a reference to its delegate (`WeakReference<DomainParticipant>::lock()`).
    explicit DomainParticipant(const BASE_T::DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A participant in domain `domain_id` with the default QoS.
    explicit DomainParticipant(uint32_t domain_id) : BASE_T(domain_id) {
        detail::ParticipantRegistry::add(domain_id, this->impl_);
    }

    /// With QoS, and optionally a listener.
    DomainParticipant(uint32_t domain_id, const ::dds::domain::qos::DomainParticipantQos& qos,
                      ::dds::domain::DomainParticipantListener* listener = 0,
                      const ::dds::core::status::StatusMask& mask =
                          ::dds::core::status::StatusMask::all())
        : BASE_T(domain_id, qos, listener, mask) {
        detail::ParticipantRegistry::add(domain_id, this->impl_);
    }

    /// The null DomainParticipant (`dds::core::null`, DDS-PSM-Cxx 1.0 §7.5.1.2).
    DomainParticipant(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    DomainParticipant(const DomainParticipant&) = default;
    DomainParticipant& operator=(const DomainParticipant&) = default;
    DomainParticipant(DomainParticipant&&) = default;
    DomainParticipant& operator=(DomainParticipant&&) = default;
    ~DomainParticipant() = default;
#endif
};

namespace detail {
struct ParticipantAccess {
    static DomainParticipant share(::zerodds::cxx::shared_ptr<DomainParticipantDelegate> d) {
        DomainParticipant dp;
        dp.impl_ = ZERODDS_MOVE(d);
        return dp;
    }
    static DomainParticipant make(::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> p) {
        DomainParticipant dp;
        dp.impl_ = ::zerodds::cxx::make_shared<DomainParticipantDelegate>(ZERODDS_MOVE(p));
        return dp;
    }
};
/// The DomainParticipant that shares `p`.
inline DomainParticipant make_participant(::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> p) {
    return ParticipantAccess::make(ZERODDS_MOVE(p));
}
} // namespace detail

/// The participant this process holds for `domain_id`
/// (DDS-PSM-Cxx 1.0 `dds::domain::find`); a null participant when none.
inline DomainParticipant find(uint32_t domain_id) {
    return detail::ParticipantAccess::share(detail::ParticipantRegistry::find(domain_id));
}

} // namespace domain
} // namespace dds

// Out-of-line definition of TopicDescription::domain_participant (needs the
// complete DomainParticipant).
namespace dds {
namespace topic {
template <typename T, typename DELEGATE>
inline ::dds::domain::DomainParticipant detail::TTopicDescription<T, DELEGATE>::domain_participant() const {
    if (!this->impl_) {
        throw ::dds::core::NullReferenceError("TopicDescription::domain_participant: null topic");
    }
    return ::dds::domain::detail::make_participant(this->delegate()->participant());
}
} // namespace topic
} // namespace dds

// The holders behind AnyTopic need the complete DomainParticipant.
#include "dds/topic/detail/AnyTopicDetail.hpp"
// The listener callbacks hand the application a complete Subscriber.
#include "dds/sub/Subscriber.hpp"
#include "dds/domain/detail/DomainParticipantListenerCallbacks.hpp"

#endif // ZERODDS_DDS_DOMAIN_DOMAINPARTICIPANT_HPP