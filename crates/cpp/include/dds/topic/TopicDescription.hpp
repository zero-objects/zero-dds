// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/TopicDescription.hpp — TopicDescription (DDS-PSM-Cxx 1.0 §7.8).

#ifndef ZERODDS_DDS_TOPIC_TOPICDESCRIPTION_HPP
#define ZERODDS_DDS_TOPIC_TOPICDESCRIPTION_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/topic/TTopicDescription.hpp"
#include "dds/topic/detail/TopicDescriptionFwd.hpp"

namespace dds {
namespace topic {

/// TopicDescription (Spec §7.8): `TTopicDescription` over a delegate.
template <typename T, typename DELEGATE>
class TopicDescription : public detail::TTopicDescription<T, DELEGATE> {
public:
    typedef detail::TTopicDescription<T, DELEGATE> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A null topic description.
protected:
    TopicDescription() ZERODDS_NOEXCEPT {}
public:

    /// A topic description sharing `ref`.
    explicit TopicDescription(const DELEGATE_REF_T& ref) ZERODDS_NOEXCEPT : BASE_T(ref) {}

#if ZERODDS_CXX11
    TopicDescription(const TopicDescription&) = default;
    TopicDescription& operator=(const TopicDescription&) = default;
    TopicDescription(TopicDescription&&) = default;
    TopicDescription& operator=(TopicDescription&&) = default;
    ~TopicDescription() = default;
#endif
};

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_TOPICDESCRIPTION_HPP
