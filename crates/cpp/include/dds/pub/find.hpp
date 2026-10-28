// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/find.hpp — DDS-PSM-Cxx 1.0 `dds::pub::find`: the writers of a
// publisher for a topic name, from the writers the publisher created and the
// application still holds.

#ifndef ZERODDS_DDS_PUB_FIND_HPP
#define ZERODDS_DDS_PUB_FIND_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <string>
#include <typeinfo>
#include <vector>

#include "dds/core/detail/borrow.hpp"
#include "dds/pub/AnyDataWriter.hpp"
#include "dds/pub/DataWriter.hpp"
#include "dds/pub/Publisher.hpp"

namespace dds {
namespace pub {
namespace detail {

/// Which of a publisher's writers a `find<WRITER>` returns, and how it makes a `WRITER` of one.
template <typename WRITER>
struct WriterFinder;

template <typename T>
struct WriterFinder<DataWriter<T> > {
    typedef ::dds::core::detail::EntityTracker<AnyDataWriter> Tracker;

    template <typename F>
    static void each(const Publisher& pub, const std::string& topic_name, F& f) {
        const std::vector<Tracker::Live> lives = pub->writers().live(&topic_name);
        for (std::vector<Tracker::Live>::const_iterator it = lives.begin(); it != lives.end(); ++it) {
            if (it->entry.sample_type == ::zerodds::cxx::type_index(typeid(T))) {
                f(::dds::core::detail::wrap_delegate<DataWriter<T> >(
                    ::zerodds::cxx::static_pointer_cast<typename DataWriter<T>::DELEGATE_T>(it->delegate)));
            }
        }
    }
};

template <>
struct WriterFinder<AnyDataWriter> {
    typedef ::dds::core::detail::EntityTracker<AnyDataWriter> Tracker;

    template <typename F>
    static void each(const Publisher& pub, const std::string& topic_name, F& f) {
        const std::vector<Tracker::Live> lives = pub->writers().live(&topic_name);
        for (std::vector<Tracker::Live>::const_iterator it = lives.begin(); it != lives.end(); ++it) {
            f(it->entry.to_any(it->delegate));
        }
    }
};

/// Copies the writers a finder visits to an iterator, at most `max_size` of them.
template <typename WRITER, typename FwdIterator>
struct WriterCollector {
    WriterCollector(FwdIterator begin, uint32_t max_size) : it_(begin), max_(max_size), n_(0) {}
    void operator()(WRITER w) {
        if (n_ < max_) {
            *it_ = ZERODDS_MOVE(w);
            ++it_;
            ++n_;
        }
    }
    FwdIterator it_;
    uint32_t max_;
    uint32_t n_;
};

} // namespace detail

/// Copies up to `max_size` of the writers of `pub` for `topic_name` to
/// `begin`; returns how many it copied.
template <typename WRITER, typename FwdIterator>
uint32_t find(const Publisher& pub, const std::string& topic_name, FwdIterator begin,
              uint32_t max_size) {
    detail::WriterCollector<WRITER, FwdIterator> collect(begin, max_size);
    detail::WriterFinder<WRITER>::each(pub, topic_name, collect);
    return collect.n_;
}

/// Copies all writers of `pub` for `topic_name` to `begin`; returns how many.
template <typename WRITER, typename BinIterator>
uint32_t find(const Publisher& pub, const std::string& topic_name, BinIterator begin) {
    detail::WriterCollector<WRITER, BinIterator> collect(begin, 0xFFFFFFFFu);
    detail::WriterFinder<WRITER>::each(pub, topic_name, collect);
    return collect.n_;
}

} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_FIND_HPP
