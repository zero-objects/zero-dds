// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/detail/Retain.hpp — DDS-PSM-Cxx 1.0 §7.5.1.2: "Any object that has been explicitly
// retained is still in use" and "Any entity with a non-null listener is still in use". The
// delegate of such an entity stays alive, with no reference held by the application, until
// `close()` of the entity or, for a listener, until the listener is removed.

#ifndef ZERODDS_DDS_CORE_DETAIL_RETAIN_HPP
#define ZERODDS_DDS_CORE_DETAIL_RETAIN_HPP


#include "zerodds/psm/cxx.hpp"
#include <map>
#include <memory>

namespace dds {
namespace core {
namespace detail {

/// Why an entity is retained: the explicit `retain()` or a listener. A plain enumeration in a
/// struct (`RetainReason::Explicit`), as the C++03 headers of the OMG do not have `enum class`.
struct RetainReason {
    enum Value { Explicit = 1, Listener = 2 };
    RetainReason(Value v) : value_(v) {}
    unsigned bits() const { return static_cast<unsigned>(value_); }

private:
    Value value_;
};

class RetainedEntities {
public:
    /// Keeps `keep` alive under `key` for `reason` until `release(key, reason)`.
    static void hold(const void* key, ::zerodds::cxx::shared_ptr<void> keep, RetainReason reason) {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex());
        Entry& e = entries()[key];
        e.keep = ZERODDS_MOVE(keep);
        e.reasons |= reason.bits();
    }

    /// Ends the retention of the entity under `key` for `reason`. The entity is let go when no
    /// reason is left; the delegate is destroyed after the last reference, outside the lock.
    static void release(const void* key, RetainReason reason) {
        ::zerodds::cxx::shared_ptr<void> dropped;
        {
            ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex());
            std::map<const void*, Entry>::iterator found = entries().find(key);
            if (found == entries().end()) {
                return;
            }
            found->second.reasons &= ~reason.bits();
            if (found->second.reasons != 0) {
                return;
            }
            dropped = found->second.keep;
            entries().erase(found);
        }
    }

    /// Ends every retention of the entity under `key` (`close()`).
    static void release_all(const void* key) {
        release(key, RetainReason::Explicit);
        release(key, RetainReason::Listener);
    }

    static bool contains(const void* key) {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(mutex());
        return entries().find(key) != entries().end();
    }

private:
    struct Entry {
        ::zerodds::cxx::shared_ptr<void> keep;
        unsigned reasons;
        Entry() : keep(), reasons(0) {}
    };
    static ::zerodds::cxx::mutex& mutex() {
        static ::zerodds::cxx::mutex* m = new ::zerodds::cxx::mutex;  // never destroyed: a retained entity outlives main
        return *m;
    }
    static std::map<const void*, Entry>& entries() {
        static std::map<const void*, Entry>* e = new std::map<const void*, Entry>;
        return *e;
    }
};

/// `retain()` of a reference type whose delegate is `d`.
template <typename D>
void retain_entity(const ::zerodds::cxx::shared_ptr<D>& d) {
    if (d) {
        RetainedEntities::hold(static_cast<const void*>(d.get()), d, RetainReason::Explicit);
    }
}

/// An entity with a listener is still in use (DDS-PSM-Cxx 1.0 §7.5.1.2): `listener(l, mask)` with
/// a non-null `l` holds it, `listener(nullptr, …)` lets it go.
template <typename D>
void retain_for_listener(const ::zerodds::cxx::shared_ptr<D>& d, bool has_listener) {
    if (!d) {
        return;
    }
    if (has_listener) {
        RetainedEntities::hold(static_cast<const void*>(d.get()), d, RetainReason::Listener);
    } else {
        RetainedEntities::release(static_cast<const void*>(d.get()), RetainReason::Listener);
    }
}

/// `close()` of a reference type whose delegate is `d`: it is no longer retained.
template <typename D>
void release_entity(const ::zerodds::cxx::shared_ptr<D>& d) {
    if (d) {
        RetainedEntities::release_all(static_cast<const void*>(d.get()));
    }
}

}  // namespace detail
}  // namespace core
}  // namespace dds

#endif  // ZERODDS_DDS_CORE_DETAIL_RETAIN_HPP
