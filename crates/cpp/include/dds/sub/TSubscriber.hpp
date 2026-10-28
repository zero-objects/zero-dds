// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/TSubscriber.hpp — DDS-PSM-Cxx 1.0 §7.10 `TSubscriber<DELEGATE>`.

#ifndef ZERODDS_DDS_SUB_TSUBSCRIBER_HPP
#define ZERODDS_DDS_SUB_TSUBSCRIBER_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>

#include "dds/core/Exception.hpp"
#include "dds/core/TEntity.hpp"
#include "dds/core/status/State.hpp"
#include "zerodds.h"
#include "dds/sub/SubscriberListener.hpp"
#include "dds/sub/qos/DataReaderQos.hpp"
#include "dds/sub/qos/SubscriberQos.hpp"

namespace dds {
namespace domain {
class DomainParticipant;
namespace detail {
DomainParticipant make_participant(::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> p);
} // namespace detail
} // namespace domain
namespace sub {
namespace detail {

/// `T`, spelled as a type that depends on `D`: a member function whose return type is
/// `DependentType<D, T>::type` is checked for the completeness of `T` where it is called, not
/// where it is defined (ISO C++03 has no default template arguments for function templates).
template <typename D, typename T>
struct DependentType {
    typedef T type;
};

} // namespace detail

/// A reference to a subscriber whose behaviour a `DELEGATE` provides.
template <typename DELEGATE>
class TSubscriber : public ::dds::core::TEntity<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef ::dds::core::TEntity<DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;
    typedef ::dds::sub::SubscriberListener Listener;

    /// A null subscriber.
protected:
    TSubscriber() ZERODDS_NOEXCEPT {}
public:

    /// A subscriber sharing `ref`.
    explicit TSubscriber(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A subscriber of `dp` with the default QoS.
    explicit TSubscriber(const ::dds::domain::DomainParticipant& dp)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(dp)) {
        this->impl_->enrol(dp, this->impl_);
    }

    /// With QoS, and optionally a listener.
    TSubscriber(const ::dds::domain::DomainParticipant& dp,
                const ::dds::sub::qos::SubscriberQos& qos,
                ::dds::sub::SubscriberListener* listener = nullptr,
                const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(dp, qos)) {
        this->impl_->enrol(dp, this->impl_);
        this->listener(listener, mask);
    }

#if ZERODDS_CXX11
    TSubscriber(const TSubscriber&) = default;
    TSubscriber& operator=(const TSubscriber&) = default;
    TSubscriber(TSubscriber&&) = default;
    TSubscriber& operator=(TSubscriber&&) = default;
    ~TSubscriber() = default;
#endif

    /// Calls `on_data_available` on the listeners of the readers that hold unread data.
    void notify_datareaders() { this->delegate()->notify_datareaders(); }

    /// Installs `l` for the statuses in `mask`; `nullptr` removes the listener.
    void listener(Listener* l, const ::dds::core::status::StatusMask& mask) {
        if (this->is_nil()) {
            throw ::dds::core::NullReferenceError("Subscriber::listener: null subscriber");
        }
        this->delegate()->listener(l, mask);
        ::dds::core::detail::retain_for_listener(this->delegate(), l != nullptr);
    }
    Listener* listener() const { return this->is_nil() ? nullptr : this->delegate()->listener(); }

    const ::dds::sub::qos::SubscriberQos& qos() const { return this->delegate()->qos(); }
    void qos(const ::dds::sub::qos::SubscriberQos& the_qos) { this->delegate()->qos(the_qos); }
    TSubscriber& operator<<(const ::dds::sub::qos::SubscriberQos& the_qos) {
        qos(the_qos);
        return *this;
    }
    TSubscriber& operator>>(::dds::sub::qos::SubscriberQos& the_qos) {
        the_qos = qos();
        return *this;
    }

    /// The QoS the readers of this subscriber get when none is given.
    ::dds::sub::qos::DataReaderQos default_datareader_qos() const {
        return this->delegate()->default_datareader_qos();
    }
    TSubscriber& default_datareader_qos(const ::dds::sub::qos::DataReaderQos& qos) {
        this->delegate()->default_datareader_qos(qos);
        return *this;
    }

    /// The participant this subscriber was made from.
    /// The return type depends on `DELEGATE` only so the complete `DomainParticipant` is needed
    /// where this is called.
    typename detail::DependentType<DELEGATE, ::dds::domain::DomainParticipant>::type participant()
        const {
        return ::dds::domain::detail::make_participant(this->delegate()->participant());
    }
};

} // namespace sub
} // namespace dds

// The member functions above need the complete DomainParticipant.
#include "dds/domain/DomainParticipant.hpp"

#endif // ZERODDS_DDS_SUB_TSUBSCRIBER_HPP
