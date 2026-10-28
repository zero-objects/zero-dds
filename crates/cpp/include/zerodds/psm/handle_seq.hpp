// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds/psm/handle_seq.hpp — the matched-handle calls of dds/pub/discovery.hpp
// and dds/sub/discovery.hpp (DDS-PSM-Cxx 1.0 `matched_subscriptions`,
// `matched_publications`): the C ABI call that grows its buffer, the copy into
// a caller's range, and the per-thread sequence the range form returns.
// Vendor helper, kept out of `dds::core` (§2.2).

#ifndef ZERODDS_PSM_HANDLE_SEQ_HPP
#define ZERODDS_PSM_HANDLE_SEQ_HPP

#include "zerodds/psm/cxx.hpp"
#include <stdint.h>
#include <vector>

#include "dds/core/InstanceHandle.hpp"
#include "zerodds/psm/check_status.hpp"

namespace zerodds {
namespace psm {
namespace handle_slots {

/// The sequence `matched_subscriptions(writer, begin, end)` and
/// `matched_publications(reader, begin, end)` return a reference to: one per
/// thread and per `Slot`, so the one call does not overwrite what the other
/// returned. A C++11 compiler has `thread_local`; for ISO C++03 the slot is a thread-specific
/// value of the platform (POSIX key, Windows fiber-local storage) that frees the sequence when
/// its thread ends.
#if ZERODDS_CXX11
template <int Slot>
inline ::dds::core::InstanceHandleSeq& thread_handle_seq() {
    static thread_local ::dds::core::InstanceHandleSeq all;
    return all;
}
#elif defined(_WIN32)
template <int Slot>
struct HandleSeqSlot {
    static VOID WINAPI destroy(PVOID p) { delete static_cast< ::dds::core::InstanceHandleSeq*>(p); }
    static ::dds::core::InstanceHandleSeq& get() {
        if (index_ == FLS_OUT_OF_INDEXES) {
            const DWORD fresh = FlsAlloc(&destroy);
            if (InterlockedCompareExchange(reinterpret_cast<volatile LONG*>(&index_),
                                           static_cast<LONG>(fresh),
                                           static_cast<LONG>(FLS_OUT_OF_INDEXES)) !=
                static_cast<LONG>(FLS_OUT_OF_INDEXES)) {
                FlsFree(fresh);
            }
        }
        void* p = FlsGetValue(index_);
        if (p == 0) {
            p = new ::dds::core::InstanceHandleSeq();
            FlsSetValue(index_, p);
        }
        return *static_cast< ::dds::core::InstanceHandleSeq*>(p);
    }
    static DWORD index_;
};
template <int Slot>
DWORD HandleSeqSlot<Slot>::index_ = FLS_OUT_OF_INDEXES;
template <int Slot>
inline ::dds::core::InstanceHandleSeq& thread_handle_seq() { return HandleSeqSlot<Slot>::get(); }
#else
template <int Slot>
struct HandleSeqSlot {
    static void destroy(void* p) { delete static_cast< ::dds::core::InstanceHandleSeq*>(p); }
    static void make_key() { pthread_key_create(&key_, &destroy); }
    static ::dds::core::InstanceHandleSeq& get() {
        pthread_once(&once_, &make_key);
        void* p = pthread_getspecific(key_);
        if (p == 0) {
            p = new ::dds::core::InstanceHandleSeq();
            pthread_setspecific(key_, p);
        }
        return *static_cast< ::dds::core::InstanceHandleSeq*>(p);
    }
    static pthread_key_t key_;
    static pthread_once_t once_;
};
template <int Slot>
pthread_key_t HandleSeqSlot<Slot>::key_;
template <int Slot>
pthread_once_t HandleSeqSlot<Slot>::once_ = PTHREAD_ONCE_INIT;
template <int Slot>
inline ::dds::core::InstanceHandleSeq& thread_handle_seq() { return HandleSeqSlot<Slot>::get(); }
#endif

} // namespace handle_slots

/// The handles a matched-entities call of the C ABI returns for `entity`
/// (`zerodds_dw_get_matched_subscriptions`, `zerodds_dr_get_matched_publications`).
/// The buffer starts at 64 and grows to the count the call reports while it
/// answers ZERODDS_OUT_OF_RESOURCES; `what` names the operation in the exception.
template <typename Native>
std::vector< ::dds::core::InstanceHandle> matched_handles(
    int (*get)(Native*, uint64_t*, uintptr_t*, uintptr_t), Native* entity, const char* what) {
    std::vector<uint64_t> raw(64);
    uintptr_t count = 0;
    int rc;
    for (;;) {
        rc = get(entity, &raw[0], &count, raw.size());
        if (rc != -8) { // ZERODDS_OUT_OF_RESOURCES
            break;
        }
        // The buffer was full: `count` is the total, grow to it and retry.
        raw.resize(count);
    }
    ::zerodds::psm::check_status(rc, what);
    std::vector< ::dds::core::InstanceHandle> handles;
    handles.reserve(count);
    for (uintptr_t i = 0; i < count; ++i) {
        handles.push_back(::dds::core::InstanceHandle(raw[i]));
    }
    return handles;
}

/// Copies `handles` to `[begin, end)` as far as the range reaches and returns
/// them as the per-thread sequence of `Slot`, which stays valid until the next
/// call with the same `Slot` on the same thread, so a caller sees how many did
/// not fit (DDS-PSM-Cxx 1.0 declares the return as a reference).
template <int Slot, typename FwdIterator>
::dds::core::InstanceHandleSeq& copy_handles(const std::vector< ::dds::core::InstanceHandle>& handles,
                                             FwdIterator begin, FwdIterator end) {
    ::dds::core::InstanceHandleSeq& all = handle_slots::thread_handle_seq<Slot>();
    all = handles;
    for (::dds::core::InstanceHandleSeq::const_iterator it = all.begin(); it != all.end(); ++it) {
        if (begin == end) {
            break;
        }
        *begin = *it;
        ++begin;
    }
    return all;
}

} // namespace psm
} // namespace zerodds

#endif // ZERODDS_PSM_HANDLE_SEQ_HPP
