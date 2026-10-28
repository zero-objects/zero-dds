// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/Topic.hpp — DDS-PSM-Cxx 1.0 §7.8.

#ifndef ZERODDS_DDS_TOPIC_TOPIC_HPP
#define ZERODDS_DDS_TOPIC_TOPIC_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "dds/core/detail/borrow.hpp"
#include <string>

#include "dds/core/Reference.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/core/TEntity.hpp"
#include "dds/topic/TopicDescription.hpp"
#include "dds/topic/TTopic.hpp"
#include "dds/topic/TopicListener.hpp"
#include "dds/topic/TopicTraits.hpp"
#include "dds/topic/detail/TopicDelegate.hpp"
#include "zerodds.h"

namespace dds {
namespace domain {
class DomainParticipant;
}
namespace core {
namespace xtypes {
namespace detail {
class DynamicData;
}  // namespace detail
template <typename DELEGATE>
class TDynamicData;
typedef TDynamicData<detail::DynamicData> DynamicData;
namespace detail {
/// Defined in dds/core/xtypes/DynamicTopic.hpp.
template <typename TYPE>
std::string register_type(const TYPE& type);
}  // namespace detail
}  // namespace xtypes
}  // namespace core

namespace topic {

/// Forward declaration for friend access.
template <typename T>
class ContentFilteredTopic;

/// Forward declaration of Topic for TopicListener.
template <typename T>
class Topic;

/// TopicListener (Spec §7.8).
template <typename T>
class Topic;

template <typename T>
class Topic : public detail::TTopic<T, detail::TopicDelegate<T> > {
public:
    typedef detail::TopicDelegate<T> DELEGATE_T;
    typedef detail::TTopic<T, DELEGATE_T> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;
    typedef TopicListener<T> Listener;
    typedef topic_type_support<T> type_support;

    /// A topic sharing `ref`.
    explicit Topic(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A topic of `dp` with the default QoS.
    Topic(const ::dds::domain::DomainParticipant& dp, const std::string& topic_name)
        : BASE_T(dp, topic_name) {}

    /// With the type registered under `type_name`.
    Topic(const ::dds::domain::DomainParticipant& dp, const std::string& topic_name,
          const std::string& type_name)
        : BASE_T(dp, topic_name, type_name) {}

    /// With QoS, and optionally a listener.
    Topic(const ::dds::domain::DomainParticipant& dp, const std::string& topic_name,
          const ::dds::topic::qos::TopicQos& qos, TopicListener<T>* listener = 0,
          const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(dp, topic_name, qos, listener, mask) {}

    Topic(const ::dds::domain::DomainParticipant& dp, const std::string& topic_name,
          const std::string& type_name, const ::dds::topic::qos::TopicQos& qos,
          TopicListener<T>* listener = 0,
          const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(dp, topic_name, type_name, qos, listener, mask) {}

    /// A null topic.
protected:
    Topic() ZERODDS_NOEXCEPT {}
public:

    /// A topic of dynamic type: `type` is a `DynamicType` (DDS-PSM-Cxx 1.0 §7.5); its name is
    /// the topic type name. Only for `Topic<DynamicData>`.
    /// The last parameter only removes the constructor from the overload set for other types.
    template <typename TYPE>
    Topic(const ::dds::domain::DomainParticipant& dp, const std::string& name, const TYPE& type,
          typename ::zerodds::cxx::enable_if<
              ::zerodds::cxx::is_same<T, ::dds::core::xtypes::DynamicData>::value &&
                  !::zerodds::cxx::is_convertible<const TYPE&, std::string>::value &&
                  !::zerodds::cxx::is_same<TYPE, ::dds::topic::qos::TopicQos>::value,
              int>::type = 0)
        : BASE_T(dp, name, ::dds::core::xtypes::detail::register_type(type)) {}

    /// The null Topic (`dds::core::null`, DDS-PSM-Cxx 1.0 §7.5.1.2).
    Topic(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    Topic(const Topic&) = default;
    Topic& operator=(const Topic&) = default;
    Topic(Topic&&) = default;
    Topic& operator=(Topic&&) = default;
    ~Topic() = default;
#endif
};

namespace detail {

/// The C callback of a topic listener: hands the event to the `TopicListener<T>`.
template <typename T>
struct TopicListenerBridge {
    static void on_inconsistent_topic(void* ud, zerodds_ZeroDdsTopic* raw) {
        TopicListener<T>* listener = static_cast<TopicListener<T>*>(ud);
        if (listener != 0) {
            Topic<T> t = ::dds::core::detail::borrow<Topic<T> >(raw);
            listener->on_inconsistent_topic(t, t.inconsistent_topic_status());
        }
    }
};

template <typename T>
inline void TopicDelegate<T>::listener(TopicListener<T>* l,
                                       const ::dds::core::status::StatusMask& mask) {
    set_listener(l);
    if (l == 0) {
        detach_listener();
        return;
    }
    zerodds_ZeroDdsTopicListener vt = zerodds_ZeroDdsTopicListener();
    vt.user_data = static_cast<void*>(l);
    vt.on_inconsistent_topic = &TopicListenerBridge<T>::on_inconsistent_topic;
    attach_listener(vt, static_cast<uint32_t>(mask.to_ulong()));
}

} // namespace detail

} // namespace topic
} // namespace dds

// Include full DomainParticipant definition for method implementations.
#include "dds/domain/DomainParticipant.hpp"

namespace dds {
namespace topic {
namespace detail {
template <typename T>
inline void TopicDelegate<T>::enrol(const ::dds::domain::DomainParticipant& dp,
                                    const ::zerodds::cxx::shared_ptr<TopicDelegate>& self) {
    dp->child_topics().add(self, DelegateCloser<TopicDelegate>());
}
} // namespace detail
} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_TOPIC_HPP