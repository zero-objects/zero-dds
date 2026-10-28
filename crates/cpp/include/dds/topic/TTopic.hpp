// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/TTopic.hpp — DDS-PSM-Cxx 1.0 §7.8 `TTopic<T, DELEGATE>`.

#ifndef ZERODDS_DDS_TOPIC_TTOPIC_HPP
#define ZERODDS_DDS_TOPIC_TTOPIC_HPP


#include "zerodds/psm/cxx.hpp"
#include <string>

#include "dds/core/Exception.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/topic/TopicDescription.hpp"
#include "dds/topic/TopicListener.hpp"
#include "dds/topic/qos/TopicQos.hpp"

namespace dds {
namespace domain {
class DomainParticipant;
}
namespace topic {

namespace detail {

/// The Topic API over any delegate.
template <typename T, typename DELEGATE>
class TTopic : public TopicDescription<T, DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef TopicDescription<T, DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;
    typedef TopicListener<T> Listener;

    /// A null topic.
protected:
    TTopic() ZERODDS_NOEXCEPT {}
public:

    /// A topic sharing `ref`.
    explicit TTopic(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A topic of `dp` with the default QoS.
    TTopic(const ::dds::domain::DomainParticipant& dp, const std::string& topic_name)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(dp, topic_name)) {
        this->impl_->enrol(dp, this->impl_);
    }

    /// With the type registered under `type_name`.
    TTopic(const ::dds::domain::DomainParticipant& dp, const std::string& topic_name,
           const std::string& type_name)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(dp, topic_name, type_name)) {
        this->impl_->enrol(dp, this->impl_);
    }

    /// With QoS, and optionally a listener.
    TTopic(const ::dds::domain::DomainParticipant& dp, const std::string& topic_name,
           const ::dds::topic::qos::TopicQos& qos, TopicListener<T>* listener = 0,
           const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(dp, topic_name, qos)) {
        this->impl_->enrol(dp, this->impl_);
        this->listener(listener, mask);
    }

    TTopic(const ::dds::domain::DomainParticipant& dp, const std::string& topic_name,
           const std::string& type_name, const ::dds::topic::qos::TopicQos& qos,
           TopicListener<T>* listener = 0,
           const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE>(dp, topic_name, type_name, qos)) {
        this->impl_->enrol(dp, this->impl_);
        this->listener(listener, mask);
    }

#if ZERODDS_CXX11
    TTopic(const TTopic&) = default;
    TTopic& operator=(const TTopic&) = default;
    TTopic(TTopic&&) = default;
    TTopic& operator=(TTopic&&) = default;
    ~TTopic() = default;
#endif

    /// Installs `l` for the statuses in `mask`; `nullptr` removes the listener.
    void listener(Listener* the_listener, const ::dds::core::status::StatusMask& event_mask) {
        if (this->is_nil()) {
            throw ::dds::core::NullReferenceError("Topic::listener: null topic");
        }
        this->delegate()->listener(the_listener, event_mask);
        ::dds::core::detail::retain_for_listener(this->delegate(), the_listener != 0);
    }
    Listener* listener() const { return this->is_nil() ? static_cast<Listener*>(0) : this->delegate()->listener(); }

    const ::dds::topic::qos::TopicQos& qos() const { return this->delegate()->qos(); }
    void qos(const ::dds::topic::qos::TopicQos& the_qos) { this->delegate()->qos(the_qos); }

    const ::dds::core::status::InconsistentTopicStatus& inconsistent_topic_status() const {
        return this->delegate()->inconsistent_topic_status();
    }
};

} // namespace detail

} // namespace topic
} // namespace dds

// `domain_participant()` and the delegate need the complete DomainParticipant.
#include "dds/domain/DomainParticipant.hpp"

#endif // ZERODDS_DDS_TOPIC_TTOPIC_HPP
