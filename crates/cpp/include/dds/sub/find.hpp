// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/find.hpp — DDS-PSM-Cxx 1.0 `dds::sub::find` and
// `dds::sub::builtin_subscriber`: the readers of a subscriber by topic name,
// by topic description or by the state of their samples, from the readers
// the subscriber created and the application still holds.

#ifndef ZERODDS_DDS_SUB_FIND_HPP
#define ZERODDS_DDS_SUB_FIND_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <algorithm>
#include <stdint.h>
#include <memory>
#include <string>
#include <typeinfo>
#include <vector>

#include "dds/core/detail/borrow.hpp"
#include "dds/domain/DomainParticipant.hpp"
#include "dds/sub/AnyDataReader.hpp"
#include "dds/sub/DataReader.hpp"
#include "dds/sub/Subscriber.hpp"
#include "dds/topic/BuiltinTopic.hpp"
#include "dds/sub/status/DataState.hpp"
#include "dds/topic/TopicDescription.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {
namespace detail {

template <typename READER>
struct ReaderFinder;

template <typename T>
struct ReaderFinder<DataReader<T> > {
    template <typename Live, typename F>
    static void emit(const Live& live, const F& f) {
        if (live.entry.sample_type == ::zerodds::cxx::type_index(typeid(T))) {
            f(::dds::core::detail::wrap_delegate<DataReader<T> >(
                ::zerodds::cxx::static_pointer_cast<typename DataReader<T>::DELEGATE_T>(live.delegate)));
        }
    }
};

template <>
struct ReaderFinder<AnyDataReader> {
    template <typename Live, typename F>
    static void emit(const Live& live, const F& f) {
        f(live.entry.to_any(live.delegate));
    }
};

/// The readers of the built-in topics are `DataReader<ParticipantBuiltinTopicData>` and the
/// three others; only these are found on the built-in subscriber.
template <typename READER>
struct IsBuiltinReader : ::zerodds::cxx::false_type {};
template <>
struct IsBuiltinReader<DataReader< ::dds::topic::ParticipantBuiltinTopicData> > : ::zerodds::cxx::true_type {};
template <>
struct IsBuiltinReader<DataReader< ::dds::topic::TopicBuiltinTopicData> > : ::zerodds::cxx::true_type {};
template <>
struct IsBuiltinReader<DataReader< ::dds::topic::PublicationBuiltinTopicData> > : ::zerodds::cxx::true_type {};
template <>
struct IsBuiltinReader<DataReader< ::dds::topic::SubscriptionBuiltinTopicData> > : ::zerodds::cxx::true_type {};

/// The reader of the built-in topic of `READER` on the built-in subscriber `sub`
/// (DDS 1.4 §2.2.2.5.2.7), when `topic_name` is null or names that topic.
template <typename READER, bool = IsBuiltinReader<READER>::value>
struct BuiltinFinder {
    template <typename F>
    static void emit(const Subscriber&, const std::string*, const F&) {}
};

template <typename READER>
struct BuiltinFinder<READER, true> {
    template <typename F>
    static void emit(const Subscriber& sub, const std::string* topic_name, const F& f) {
        if (topic_name != 0 && *topic_name != READER::topic_name()) {
            return;
        }
        zerodds_ZeroDdsBuiltinReader* handle =
            zerodds_builtin_subscriber_lookup_datareader(sub->builtin_handle(),
                                                         READER::topic_name());
        if (handle != 0) {
            f(READER(handle));
        }
    }
};

/// The readers `sub` created and the application still holds. A reader of a built-in topic is
/// not one of them.
template <typename READER, bool = IsBuiltinReader<READER>::value>
struct LiveReaders {
    template <typename F>
    static void each(const Subscriber& sub, const std::string* topic_name, const F& f) {
        typedef ::dds::core::detail::EntityTracker< ::dds::sub::AnyDataReader> Tracker;
        const std::vector<Tracker::Live> lives = sub->readers().live(topic_name);
        for (std::vector<Tracker::Live>::const_iterator live = lives.begin(); live != lives.end();
             ++live) {
            ReaderFinder<READER>::emit(*live, f);
        }
    }
};

template <typename READER>
struct LiveReaders<READER, true> {
    template <typename F>
    static void each(const Subscriber&, const std::string*, const F&) {}
};

/// The readers of `sub` for `topic_name` (all readers when null), in creation order. The
/// built-in subscriber has the readers of the four built-in topics.
template <typename READER, typename F>
void each_reader(const Subscriber& sub, const std::string* topic_name, const F& f) {
    if (sub->builtin_handle() != 0) {
        BuiltinFinder<READER>::emit(sub, topic_name, f);
        return;
    }
    LiveReaders<READER>::each(sub, topic_name, f);
}

/// The C handles of the readers of `sub` that hold samples in `state`.
inline std::vector<const void*> readers_with_state(const Subscriber& sub,
                                                   const ::dds::sub::status::DataState& state) {
    const uint32_t ss = static_cast<uint32_t>(state.sample_state().to_ulong());
    const uint32_t vs = static_cast<uint32_t>(state.view_state().to_ulong());
    const uint32_t is = static_cast<uint32_t>(state.instance_state().to_ulong());
    std::vector<zerodds_ZeroDdsDataReader*> raw(16, static_cast<zerodds_ZeroDdsDataReader*>(0));
    uintptr_t count = 0;
    int rc;
    for (;;) {
        rc = zerodds_sub_get_datareaders(sub->native_handle(), &raw[0], &count, raw.size(), ss,
                                         vs, is);
        if (rc != -8) { // ZERODDS_OUT_OF_RESOURCES: `count` is the total
            break;
        }
        raw.resize(count);
    }
    ::zerodds::psm::check_status(rc, "find: get_datareaders");
    return std::vector<const void*>(raw.begin(), raw.begin() + count);
}

/// Collects the readers `each_reader` finds: at most `max_size` of them, to `*begin`.
template <typename READER, typename FwdIterator>
class StoreLimited {
public:
    StoreLimited(FwdIterator& begin, uint32_t& n, uint32_t max_size)
        : begin_(&begin), n_(&n), max_size_(max_size) {}
    void operator()(READER r) const {
        if (*n_ < max_size_) {
            **begin_ = ZERODDS_MOVE(r);
            ++*begin_;
            ++*n_;
        }
    }

private:
    FwdIterator* begin_;
    uint32_t* n_;
    uint32_t max_size_;
};

/// Collects every reader `each_reader` finds, to `*begin`.
template <typename READER, typename BinIterator>
class StoreAll {
public:
    StoreAll(BinIterator& begin, uint32_t& n) : begin_(&begin), n_(&n) {}
    void operator()(READER r) const {
        **begin_ = ZERODDS_MOVE(r);
        ++*begin_;
        ++*n_;
    }

private:
    BinIterator* begin_;
    uint32_t* n_;
};

/// Hands the readers of `sub` that hold samples in `state` to `f`, in creation order.
template <typename READER, typename F>
void each_reader_with_state(const Subscriber& sub, const ::dds::sub::status::DataState& state,
                            const F& f) {
    typedef ::dds::core::detail::EntityTracker< ::dds::sub::AnyDataReader> Tracker;
    const std::vector<const void*> handles = readers_with_state(sub, state);
    const std::vector<Tracker::Live> lives = sub->readers().live();
    for (std::vector<Tracker::Live>::const_iterator live = lives.begin(); live != lives.end();
         ++live) {
        if (std::find(handles.begin(), handles.end(), live->entry.native(live->delegate)) ==
            handles.end()) {
            continue;
        }
        ReaderFinder<READER>::emit(*live, f);
    }
}

} // namespace detail

/// The built-in subscriber of `dp` (DDS 1.4 §2.2.5), a `Subscriber` whose readers are those of
/// the built-in topics `DCPSParticipant`, `DCPSTopic`, `DCPSPublication` and `DCPSSubscription`,
/// found with `dds::sub::find`.
inline Subscriber builtin_subscriber(const ::dds::domain::DomainParticipant& dp) {
    return ::dds::core::detail::wrap_delegate<Subscriber>(
        ::zerodds::cxx::make_shared<Subscriber::DELEGATE_T>(dp, dp->get_builtin_subscriber()));
}

/// Readers of `sub` for `topic_name`, at most `max_size`, to `begin`; returns the count.
template <typename READER, typename FwdIterator>
uint32_t find(const Subscriber& sub, const std::string& topic_name, FwdIterator begin,
              uint32_t max_size) {
    uint32_t n = 0;
    detail::each_reader<READER>(sub, &topic_name,
                                detail::StoreLimited<READER, FwdIterator>(begin, n, max_size));
    return n;
}

/// Readers of `sub` for `topic_name` to `begin`; returns the count.
template <typename READER, typename BinIterator>
uint32_t find(const Subscriber& sub, const std::string& topic_name, BinIterator begin) {
    uint32_t n = 0;
    detail::each_reader<READER>(sub, &topic_name, detail::StoreAll<READER, BinIterator>(begin, n));
    return n;
}

/// Readers of `sub` for `topic_description`, at most `max_size`, to `begin`.
template <typename READER, typename T, typename D, typename FwdIterator>
uint32_t find(const Subscriber& sub, const ::dds::topic::TopicDescription<T, D>& topic_description,
              FwdIterator begin, uint32_t max_size) {
    return find<READER>(sub, topic_description.name(), begin, max_size);
}

/// Readers of `sub` for `topic_description` to `begin`.
template <typename READER, typename T, typename D, typename BinIterator>
uint32_t find(const Subscriber& sub, const ::dds::topic::TopicDescription<T, D>& topic_description,
              BinIterator begin) {
    return find<READER>(sub, topic_description.name(), begin);
}

/// Readers of `sub` that hold samples in `state`, at most `max_size`, to `begin`.
template <typename READER, typename FwdIterator>
uint32_t find(const Subscriber& sub, const ::dds::sub::status::DataState& state, FwdIterator begin,
              uint32_t max_size) {
    uint32_t n = 0;
    detail::each_reader_with_state<READER>(sub, state,
                                           detail::StoreLimited<READER, FwdIterator>(begin, n, max_size));
    return n;
}

/// Readers of `sub` that hold samples in `state` to `begin`.
template <typename READER, typename BinIterator>
uint32_t find(const Subscriber& sub, const ::dds::sub::status::DataState& state,
              BinIterator begin) {
    uint32_t n = 0;
    detail::each_reader_with_state<READER>(sub, state, detail::StoreAll<READER, BinIterator>(begin, n));
    return n;
}

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_FIND_HPP
