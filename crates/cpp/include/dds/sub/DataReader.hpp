// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/DataReader.hpp — DDS-PSM-Cxx 1.0 §7.10.

#ifndef ZERODDS_DDS_SUB_DATAREADER_HPP
#define ZERODDS_DDS_SUB_DATAREADER_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <memory>
#include <utility>
#include <vector>

#include "dds/core/Reference.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/Time.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/core/TEntity.hpp"
#include "dds/sub/LoanedSamples.hpp"
#include "dds/sub/SharedSamples.hpp"
#include "dds/sub/Query.hpp"
#include "dds/topic/AnyTopicDescription.hpp"
#include "dds/topic/TopicInstance.hpp"
#include "dds/sub/DataReaderListener.hpp"
#include "dds/sub/TDataReader.hpp"
#include "dds/sub/detail/DataReaderDelegate.hpp"
#include "dds/sub/detail/DataReaderFwd.hpp"
#include "dds/sub/status/DataState.hpp"
#include "dds/core/detail/borrow.hpp"
#include "dds/sub/Subscriber.hpp"
#include "dds/topic/Topic.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {
namespace detail {

/// The `ANY` of a tracked reader delegate (`AnyDataReader` for the delegate of a `READER`).
template <typename READER, typename ANY>
struct TrackedReaderToAny {
    ANY operator()(const ::zerodds::cxx::shared_ptr<void>& d) const {
        return ANY(::dds::core::detail::wrap_delegate<READER>(
            ::zerodds::cxx::static_pointer_cast<typename READER::DELEGATE_T>(d)));
    }
};

/// The C handle of a tracked reader delegate.
template <typename DELEGATE_T>
struct TrackedReaderNative {
    const void* operator()(const ::zerodds::cxx::shared_ptr<void>& d) const {
        return static_cast<DELEGATE_T*>(d.get())->native_handle();
    }
};

} // namespace detail

/// DataReader<T> (Spec §7.10).
/// Reference type with delegate pattern (DDS-PSM-Cxx 1.0 §7.5.1.1, §7.5.1.2):
/// `TDataReader` over ZeroDDS's delegate.
template <typename T, template <typename Q> class DELEGATE>
class DataReader : public detail::TDataReader<T, DELEGATE<T> > {
public:
    typedef DELEGATE<T> DELEGATE_T;
    typedef detail::TDataReader<T, DELEGATE_T> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;
    typedef DataReaderListener<T> Listener;

    /// A null reader.
protected:
    DataReader() ZERODDS_NOEXCEPT {}
public:

    /// A reader sharing `ref`.
    explicit DataReader(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A reader of `topic` with the default QoS.
    DataReader(const Subscriber& sub, const ::dds::topic::Topic<T>& topic) : BASE_T(sub, topic) {
        track(sub, topic.name());
    }

    /// With QoS, and optionally a listener.
    DataReader(const Subscriber& sub, const ::dds::topic::Topic<T>& topic,
               const ::dds::sub::qos::DataReaderQos& qos, DataReaderListener<T>* listener = nullptr,
               const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(sub, topic, qos, listener, mask) {
        track(sub, topic.name());
    }

    /// A reader of a ContentFilteredTopic (Spec §7.8): the filter is evaluated on every
    /// read and take. Templated on the topic type to avoid a hard include cycle; any
    /// `dds::topic::ContentFilteredTopic<T>` works.
    template <typename CftT>
    DataReader(const Subscriber& sub, const CftT& cft) : BASE_T(sub, cft) {
        track(sub, cft.name());
    }

    template <typename CftT>
    DataReader(const Subscriber& sub, const CftT& cft, const ::dds::sub::qos::DataReaderQos& qos,
               DataReaderListener<T>* listener = nullptr,
               const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(sub, cft, qos, listener, mask) {
        track(sub, cft.name());
    }

    /// The null DataReader (`dds::core::null`, DDS-PSM-Cxx 1.0 §7.5.1.2).
    DataReader(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    DataReader(const DataReader&) = default;
    DataReader& operator=(const DataReader&) = default;
    DataReader(DataReader&&) = default;
    DataReader& operator=(DataReader&&) = default;
    ~DataReader() = default;
#endif

private:
    /// Registers the reader with its subscriber, where `dds::sub::find` finds it.
    void track(const Subscriber& sub, const std::string& topic_name) {
        typedef ::dds::core::detail::EntityTracker<AnyDataReader> Tracker;
        typename Tracker::Entry e = {
            topic_name, ::zerodds::cxx::type_index(typeid(T)), ::zerodds::cxx::weak_ptr<void>(this->impl_),
            detail::TrackedReaderToAny<DataReader<T>, AnyDataReader>(),
            detail::TrackedReaderNative<DELEGATE_T>()};
        sub->readers().add(ZERODDS_MOVE(e));
    }
};


namespace functors {

/// Manipulator functors of the streaming API (§7.5.4 `dds/sub/DataReader.hpp`).
class MaxSamplesManipulatorFunctor {
public:
    explicit MaxSamplesManipulatorFunctor(uint32_t n) : n_(n) {}
    template <typename S>
    void operator()(S& selector) const { selector.max_samples(n_); }
private:
    uint32_t n_;
};
class ContentFilterManipulatorFunctor {
public:
    explicit ContentFilterManipulatorFunctor(const Query& q) : query_(q) {}
    template <typename S>
    void operator()(S& selector) const { selector.content(query_); }
private:
    Query query_;
};
class StateFilterManipulatorFunctor {
public:
    explicit StateFilterManipulatorFunctor(const status::DataState& s) : state_(s) {}
    template <typename S>
    void operator()(S& selector) const { selector.state(state_); }
private:
    status::DataState state_;
};
class InstanceManipulatorFunctor {
public:
    explicit InstanceManipulatorFunctor(const ::dds::core::InstanceHandle& h) : handle_(h) {}
    template <typename S>
    void operator()(S& selector) const { selector.instance(handle_); }
private:
    ::dds::core::InstanceHandle handle_;
};
class NextInstanceManipulatorFunctor {
public:
    explicit NextInstanceManipulatorFunctor(const ::dds::core::InstanceHandle& h) : handle_(h) {}
    template <typename S>
    void operator()(S& selector) const { selector.next_instance(handle_); }
private:
    ::dds::core::InstanceHandle handle_;
};

} // namespace functors

/// `dr >> read >> samples` reads, `dr >> take >> samples` takes.
template <typename SELECTOR>
SELECTOR& read(SELECTOR& selector) {
    selector.read_mode(true);
    return selector;
}
template <typename SELECTOR>
SELECTOR& take(SELECTOR& selector) {
    selector.read_mode(false);
    return selector;
}

inline functors::MaxSamplesManipulatorFunctor max_samples(uint32_t n) {
    return functors::MaxSamplesManipulatorFunctor(n);
}
inline functors::ContentFilterManipulatorFunctor content(const Query& query) {
    return functors::ContentFilterManipulatorFunctor(query);
}
inline functors::StateFilterManipulatorFunctor state(const status::DataState& s) {
    return functors::StateFilterManipulatorFunctor(s);
}
inline functors::InstanceManipulatorFunctor instance(const ::dds::core::InstanceHandle& h) {
    return functors::InstanceManipulatorFunctor(h);
}
inline functors::NextInstanceManipulatorFunctor next_instance(const ::dds::core::InstanceHandle& h) {
    return functors::NextInstanceManipulatorFunctor(h);
}

} // namespace sub
} // namespace dds

namespace dds {
namespace sub {
namespace detail {

template <typename T>
inline void DataReaderDelegate<T>::listener(DataReaderListener<T>* l,
                                            const ::dds::core::status::StatusMask& mask) {
    set_listener(l);
    if (l == nullptr) {
        detach_listener();
        return;
    }
    attach_listener(DataReaderListenerBridge<T>::table(l), static_cast<uint32_t>(mask.to_ulong()));
}

} // namespace detail
} // namespace sub
} // namespace dds

#include "dds/sub/BuiltinDataReader.hpp"

// AnyDataReader holds a DataReader<T> of any T.
#include "dds/sub/detail/AnyDataReaderDetail.hpp"

#endif // ZERODDS_DDS_SUB_DATAREADER_HPP