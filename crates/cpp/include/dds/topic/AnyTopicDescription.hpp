// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/AnyTopicDescription.hpp — AnyTopicDescription (DDS-PSM-Cxx 1.0
// §7.5.3): a Topic, ContentFilteredTopic or MultiTopic of any topic type.

#ifndef ZERODDS_DDS_TOPIC_ANYTOPICDESCRIPTION_HPP
#define ZERODDS_DDS_TOPIC_ANYTOPICDESCRIPTION_HPP


#include "zerodds/psm/cxx.hpp"
#include <string>
#include <typeinfo>

#include "dds/core/Exception.hpp"
#include "dds/topic/detail/TopicDescriptionFwd.hpp"

namespace dds {
namespace domain {
class DomainParticipant;
} // namespace domain

namespace topic {

namespace detail {

class TDHolderBase {
public:
    virtual ~TDHolderBase() {}
    virtual const ::dds::domain::DomainParticipant& domain_participant() const = 0;
    virtual const std::string& name() const = 0;
    virtual const std::string& type_name() const = 0;
    virtual const std::type_info& type() const = 0;
    /// The description as a `void*` to the `TopicDescription<T, D>` it holds.
    virtual const void* description() const = 0;
};

template <typename T, typename DELEGATE>
class TDHolder;

} // namespace detail

class AnyTopicDescription {
public:
    template <typename T, typename DELEGATE>
    AnyTopicDescription(const TopicDescription<T, DELEGATE>& t);

    const ::dds::domain::DomainParticipant& domain_participant() const {
        return holder().domain_participant();
    }
    const std::string& name() const { return holder().name(); }
    const std::string& type_name() const { return holder().type_name(); }

    AnyTopicDescription& swap(AnyTopicDescription& rhs) {
        holder_.swap(rhs.holder_);
        return *this;
    }

    /// The description as the `TopicDescription<T, DELEGATE>` it was made
    /// from; `InvalidDowncastError` for another type.
    template <typename T, typename DELEGATE>
    const TopicDescription<T, DELEGATE>& get() const {
        const detail::TDHolderBase& h = holder();
        if (h.type() != typeid(TopicDescription<T, DELEGATE>)) {
            throw ::dds::core::InvalidDowncastError("AnyTopicDescription::get");
        }
        return *static_cast<const TopicDescription<T, DELEGATE>*>(h.description());
    }

    const detail::TDHolderBase* operator->() const { return &holder(); }
    detail::TDHolderBase* operator->() { return &holder(); }

private:
    detail::TDHolderBase& holder() const {
        if (!holder_) {
            throw ::dds::core::NullReferenceError("AnyTopicDescription: null");
        }
        return *holder_;
    }

    ::zerodds::cxx::shared_ptr<detail::TDHolderBase> holder_;
};

} // namespace topic
} // namespace dds

// The holders behind it need the complete TopicDescription and DomainParticipant.
#include "dds/topic/TopicDescription.hpp"

#endif // ZERODDS_DDS_TOPIC_ANYTOPICDESCRIPTION_HPP
