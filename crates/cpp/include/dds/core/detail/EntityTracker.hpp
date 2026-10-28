// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/detail/EntityTracker.hpp — what `dds::pub::find` and
// `dds::sub::find` search: the endpoints a publisher or a subscriber created
// and the application still holds. The tracker keeps weak references only,
// so it never keeps an endpoint alive.

#ifndef ZERODDS_DDS_CORE_DETAIL_ENTITYTRACKER_HPP
#define ZERODDS_DDS_CORE_DETAIL_ENTITYTRACKER_HPP


#include "zerodds/psm/cxx.hpp"
#include <algorithm>
#include <memory>
#include <string>
#include <utility>
#include <vector>

namespace dds {
namespace core {
namespace detail {

/// `AnyT` is the type-erased endpoint (`AnyDataWriter`, `AnyDataReader`).
template <typename AnyT>
class EntityTracker {
public:
    struct Entry {
        std::string topic_name;
        ::zerodds::cxx::type_index sample_type;
        ::zerodds::cxx::weak_ptr<void> delegate;
        /// The `AnyT` for the delegate.
        ::zerodds::cxx::function<AnyT(const ::zerodds::cxx::shared_ptr<void>&)> to_any;
        /// The C handle of the delegate, for comparison with handles the C API returns.
        ::zerodds::cxx::function<const void*(const ::zerodds::cxx::shared_ptr<void>&)> native;
    };

    /// An entry whose delegate is still alive, with the delegate locked.
    struct Live {
        Entry entry;
        ::zerodds::cxx::shared_ptr<void> delegate;
        Live(const Entry& e, const ::zerodds::cxx::shared_ptr<void>& d) : entry(e), delegate(d) {}
    };

    void add(Entry e) {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
        entries_.erase(std::remove_if(entries_.begin(), entries_.end(), Expired()), entries_.end());
        entries_.push_back(ZERODDS_MOVE(e));
    }

    /// The live entries in creation order; with `topic_name` only those of that topic.
    std::vector<Live> live(const std::string* topic_name = nullptr) const {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
        std::vector<Live> out;
        for (typename std::vector<Entry>::const_iterator it = entries_.begin(); it != entries_.end();
             ++it) {
            const Entry& e = *it;
            if (topic_name != nullptr && e.topic_name != *topic_name) {
                continue;
            }
            ::zerodds::cxx::shared_ptr<void> d = e.delegate.lock();
            if (d) {
                out.push_back(Live(e, d));
            }
        }
        return out;
    }

private:
    struct Expired {
        bool operator()(const Entry& x) const { return x.delegate.expired(); }
    };

    mutable ::zerodds::cxx::mutex mutex_;
    std::vector<Entry> entries_;
};

} // namespace detail
} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_DETAIL_ENTITYTRACKER_HPP
