// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/detail/ChildTracker.hpp — the entities an entity contains, for
// `delete_contained_entities` (DDS 1.4 §2.2.2.2.1.10, §2.2.2.4.1.5, §2.2.2.5.2.18). A child is
// tracked weakly: an entity the application dropped needs no closing. The C API frees the
// children of a participant, a publisher or a subscriber; the delegates that stand for them
// close them first, in the order that leaves no dangling parent.

#ifndef ZERODDS_DDS_CORE_DETAIL_CHILDTRACKER_HPP
#define ZERODDS_DDS_CORE_DETAIL_CHILDTRACKER_HPP


#include "zerodds/psm/cxx.hpp"
#include <algorithm>
#include <memory>
#include <utility>
#include <vector>

namespace dds {
namespace core {
namespace detail {

class ChildTracker {
public:
    typedef ::zerodds::cxx::function<void(const ::zerodds::cxx::shared_ptr<void>&)> Closer;

    /// Tracks `child`; `closer` closes it (and what it contains).
    void add(const ::zerodds::cxx::shared_ptr<void>& child, Closer closer) {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
        entries_.erase(std::remove_if(entries_.begin(), entries_.end(), Expired()), entries_.end());
        Entry entry;
        entry.child = child;
        entry.closer = ZERODDS_MOVE(closer);
        entries_.push_back(entry);
    }

    /// Closes every child that is still alive, the one added last first. An exception of one
    /// closer does not keep the others from running; the first one is thrown at the end.
    void close_all() {
        std::vector<Entry> snapshot;
        {
            ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex_);
            snapshot.swap(entries_);
        }
        close_from(snapshot, snapshot.size());
    }

private:
    struct Entry {
        ::zerodds::cxx::weak_ptr<void> child;
        Closer closer;
    };
    struct Expired {
        bool operator()(const Entry& e) const { return e.child.expired(); }
    };

    /// Closes `entries[n - 1]` down to `entries[0]`. At the first exception the rest is closed
    /// (their exceptions are dropped), then the first one is thrown on.
    static void close_from(const std::vector<Entry>& entries, std::size_t n) {
        for (std::size_t k = n; k > 0; --k) {
            const Entry& entry = entries[k - 1];
            ::zerodds::cxx::shared_ptr<void> child = entry.child.lock();
            if (child) {
                try {
                    entry.closer(child);
                } catch (...) {
                    close_quietly(entries, k - 1);
                    throw;
                }
            }
        }
    }
    static void close_quietly(const std::vector<Entry>& entries, std::size_t n) {
        for (std::size_t k = n; k > 0; --k) {
            const Entry& entry = entries[k - 1];
            ::zerodds::cxx::shared_ptr<void> child = entry.child.lock();
            if (child) {
                try {
                    entry.closer(child);
                } catch (...) {
                }
            }
        }
    }
    ::zerodds::cxx::mutex mutex_;
    std::vector<Entry> entries_;
};

}  // namespace detail
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_DETAIL_CHILDTRACKER_HPP
