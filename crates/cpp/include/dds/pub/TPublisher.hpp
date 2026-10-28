// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/TPublisher.hpp — DDS-PSM-Cxx 1.0 §7.9 `TPublisher<DELEGATE>`: the
// Publisher API over any delegate that offers the same operations.

#ifndef ZERODDS_DDS_PUB_TPUBLISHER_HPP
#define ZERODDS_DDS_PUB_TPUBLISHER_HPP


#include "zerodds/psm/cxx.hpp"

#include "dds/core/Duration.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/TEntity.hpp"
#include "dds/core/status/State.hpp"
#include "zerodds.h"
#include "dds/pub/PublisherListener.hpp"
#include "dds/pub/qos/DataWriterQos.hpp"
#include "dds/pub/qos/PublisherQos.hpp"

namespace dds {
namespace domain {
class DomainParticipant;
namespace detail {
DomainParticipant make_participant(::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> p);
} // namespace detail
} // namespace domain
namespace pub {
namespace detail {
template <typename DELEGATE>
struct participant_of {
    typedef ::dds::domain::DomainParticipant type;
};
} // namespace detail

/// A reference to a publisher whose behaviour a `DELEGATE` provides.
template <typename DELEGATE>
class TPublisher : public ::dds::core::TEntity<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef ::dds::core::TEntity<DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;
    typedef ::dds::pub::PublisherListener Listener;

    /// A null publisher.
protected:
    TPublisher() ZERODDS_NOEXCEPT {}
public:

    /// A publisher sharing `ref`.
    explicit TPublisher(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A publisher of `dp` with the default QoS.
    explicit TPublisher(const ::dds::domain::DomainParticipant& dp)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(dp)) {
        this->impl_->enrol(dp, this->impl_);
    }

    /// With QoS, and optionally a listener.
    TPublisher(const ::dds::domain::DomainParticipant& dp, const ::dds::pub::qos::PublisherQos& qos,
               ::dds::pub::PublisherListener* listener = 0,
               const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(dp, qos)) {
        this->impl_->enrol(dp, this->impl_);
        this->listener(listener, mask);
    }

#if ZERODDS_CXX11
    TPublisher(const TPublisher&) = default;
    TPublisher& operator=(const TPublisher&) = default;
    TPublisher(TPublisher&&) = default;
    TPublisher& operator=(TPublisher&&) = default;
    ~TPublisher() = default;
#endif

    const ::dds::pub::qos::PublisherQos qos() const { return this->delegate()->qos(); }
    void qos(const ::dds::pub::qos::PublisherQos& pqos) { this->delegate()->qos(pqos); }
    TPublisher& operator<<(const ::dds::pub::qos::PublisherQos& the_qos) {
        qos(the_qos);
        return *this;
    }
    TPublisher& operator>>(::dds::pub::qos::PublisherQos& the_qos) {
        the_qos = qos();
        return *this;
    }

    /// The QoS the writers of this publisher get when none is given.
    TPublisher& default_writer_qos(const ::dds::pub::qos::DataWriterQos& dwqos) {
        this->delegate()->default_writer_qos(dwqos);
        return *this;
    }
    ::dds::pub::qos::DataWriterQos default_writer_qos() const {
        return this->delegate()->default_writer_qos();
    }

    /// Installs `l` for the statuses in `mask`; `nullptr` removes the listener.
    void listener(Listener* l, const ::dds::core::status::StatusMask& mask) {
        if (this->is_nil()) {
            throw ::dds::core::NullReferenceError("Publisher::listener: null publisher");
        }
        this->delegate()->listener(l, mask);
        ::dds::core::detail::retain_for_listener(this->delegate(), l != 0);
    }
    Listener* listener() const { return this->is_nil() ? 0 : this->delegate()->listener(); }

    void wait_for_acknowledgments(const ::dds::core::Duration& timeout) {
        this->delegate()->wait_for_acknowledgments(timeout);
    }

    /// The participant this publisher was made from.
    /// The result type depends on `DELEGATE`, so the complete `DomainParticipant` is needed only
    /// where this is called.
    typename detail::participant_of<DELEGATE>::type participant() const {
        return ::dds::domain::detail::make_participant(this->delegate()->participant());
    }
};

} // namespace pub
} // namespace dds

// The member functions above need the complete DomainParticipant.
#include "dds/domain/DomainParticipant.hpp"

#endif // ZERODDS_DDS_PUB_TPUBLISHER_HPP
