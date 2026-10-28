// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/detail/TopicDescriptionFwd.hpp — declaration of
// `dds::topic::TopicDescription` with its default delegate, so
// `TopicDescription<T>` (DDS-PSM-Cxx 1.0) names a topic description of `T`.

#ifndef ZERODDS_DDS_TOPIC_DETAIL_TOPICDESCRIPTIONFWD_HPP
#define ZERODDS_DDS_TOPIC_DETAIL_TOPICDESCRIPTIONFWD_HPP

#include "zerodds/psm/cxx.hpp"

namespace dds {
namespace topic {
namespace detail {
template <typename T>
class TopicDelegate;

/// The closer a participant keeps for a topic description it tracks (`ChildTracker::add`): closes
/// the delegate `D` the type-erased child points at.
template <typename D>
struct DelegateCloser {
    void operator()(const ::zerodds::cxx::shared_ptr<void>& child) const {
        ::zerodds::cxx::static_pointer_cast<D>(child)->close();
    }
};
} // namespace detail

template <typename T, typename DELEGATE = detail::TopicDelegate<T> >
class TopicDescription;

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_DETAIL_TOPICDESCRIPTIONFWD_HPP
