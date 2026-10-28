// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/TDataReader.hpp — DDS-PSM-Cxx 1.0 §7.10 `TDataReader<T, DELEGATE>`.

#ifndef ZERODDS_DDS_SUB_TDATAREADER_HPP
#define ZERODDS_DDS_SUB_TDATAREADER_HPP


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
#include "dds/sub/detail/ReadSpec.hpp"
#include "dds/sub/status/DataState.hpp"
#include "zerodds.h"

namespace dds {
namespace topic {
template <typename T>
class Topic;
}
namespace sub {
class Subscriber;
template <typename T>
class DataReaderListener;

namespace detail {

/// The DataReader API over any delegate (Spec §7.10).
template <typename T, typename DELEGATE>
class TDataReader : public ::dds::core::TEntity<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef DataReaderListener<T> Listener;
    typedef ::dds::core::TEntity<DELEGATE_T> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A null reader.
protected:
    TDataReader() ZERODDS_NOEXCEPT {}
public:

    /// A reader sharing `ref`.
    explicit TDataReader(const typename BASE_T::DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// Constructed via Sub + Topic with default QoS.
    TDataReader(const Subscriber& sub, const ::dds::topic::Topic<T>& topic)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE_T>(sub, topic)) {
    }

    /// With QoS, and optionally a listener (§7.5.4 `TDataReader`).
    TDataReader(const Subscriber& sub, const ::dds::topic::Topic<T>& topic,
               const ::dds::sub::qos::DataReaderQos& qos, DataReaderListener<T>* listener = nullptr,
               const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE_T>(sub, topic, qos)) {
        this->listener(listener, mask);
    }

    // CFT-aware constructors (Spec §7.8) — binds the reader to a
    // ContentFilteredTopic so its filter expression is evaluated on every
    // take/read. Templated on the CFT type to avoid a hard include cycle;
    // any type exposing `native_cft_handle()` works (i.e.
    // `dds::topic::ContentFilteredTopic<T>`).
    template <typename CftT>
    TDataReader(const Subscriber& sub, const CftT& cft)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE_T>(sub, cft)) {
    }

    template <typename CftT>
    TDataReader(const Subscriber& sub, const CftT& cft, const ::dds::sub::qos::DataReaderQos& qos,
               DataReaderListener<T>* listener = nullptr,
               const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE_T>(sub, cft, qos)) {
        this->listener(listener, mask);
    }

    // Reference semantics: copy/assign share the delegate.
#if ZERODDS_CXX11
    TDataReader(const TDataReader&) = default;
    TDataReader& operator=(const TDataReader&) = default;
    TDataReader(TDataReader&&) = default;
    TDataReader& operator=(TDataReader&&) = default;
    ~TDataReader() = default;
#endif

    // ---- Reading and taking (DDS-PSM-Cxx 1.0 §7.5.4 TDataReader) ----

    /// The default state filter of `read`, `take` and the streaming operators
    /// (initially `DataState::any()`).
    const ::dds::sub::status::DataState& default_filter_state() {
        return this->delegate()->default_filter_state();
    }
    TDataReader& default_filter_state(const ::dds::sub::status::DataState& state) {
        this->delegate()->default_filter_state(state);
        return *this;
    }

    /// Selects what a read or take returns: `dr.select().max_samples(10).take()`.
    class Selector {
    public:
        Selector(TDataReader& dr) : dr_(dr) { spec_.state = dr.default_filter_state(); }

        Selector& instance(const ::dds::core::InstanceHandle& h) {
            spec_.scope = detail::ReadSpec::Scope::Instance;
            spec_.instance = h;
            return *this;
        }
        Selector& next_instance(const ::dds::core::InstanceHandle& h) {
            spec_.scope = detail::ReadSpec::Scope::NextInstance;
            spec_.instance = h;
            return *this;
        }
        Selector& state(const ::dds::sub::status::DataState& s) {
            spec_.state = s;
            return *this;
        }
        Selector& content(const ::dds::sub::Query& query) {
            query_ = ::zerodds::cxx::make_shared< ::dds::sub::Query>(query);
            spec_.query = query_.get();
            return *this;
        }
        Selector& max_samples(uint32_t n) {
            spec_.max_samples = n;
            return *this;
        }

        ::dds::sub::LoanedSamples<T> read() { return dr_.delegate()->read(spec_); }
        ::dds::sub::LoanedSamples<T> take() { return dr_.delegate()->take(spec_); }

        // Forward iterators: at most `max_samples` samples are written.
        template <typename SamplesFWIterator>
        uint32_t read(SamplesFWIterator sfit, uint32_t max_samples) {
            return copy_out(sfit, max_samples, false);
        }
        template <typename SamplesFWIterator>
        uint32_t take(SamplesFWIterator sfit, uint32_t max_samples) {
            return copy_out(sfit, max_samples, true);
        }
        // Back-inserting iterators: every sample.
        template <typename SamplesBIIterator>
        uint32_t read(SamplesBIIterator sbit) {
            return copy_out(sbit, spec_.max_samples, false);
        }
        template <typename SamplesBIIterator>
        uint32_t take(SamplesBIIterator sbit) {
            return copy_out(sbit, spec_.max_samples, true);
        }

    private:
        template <typename It>
        uint32_t copy_out(It it, uint32_t max_samples, bool take) {
            detail::ReadSpec spec = spec_;
            spec.max_samples = max_samples;
            std::vector< ::dds::sub::Sample<T> > samples = dr_.delegate()->fetch(spec, take);
            for (typename std::vector< ::dds::sub::Sample<T> >::iterator s = samples.begin();
                 s != samples.end(); ++s) {
                *it = ZERODDS_MOVE(*s);
                ++it;
            }
            return static_cast<uint32_t>(samples.size());
        }

        TDataReader& dr_;
        detail::ReadSpec spec_;
        ::zerodds::cxx::shared_ptr< ::dds::sub::Query> query_;
    };

    /// The selector of the streaming operators `dr >> read >> samples`.
    class ManipulatorSelector {
    public:
        ManipulatorSelector(TDataReader& dr) : selector_(dr), read_mode_(false) {}

        bool read_mode() { return read_mode_; }
        void read_mode(bool b) { read_mode_ = b; }

        ManipulatorSelector& max_samples(uint32_t n) {
            selector_.max_samples(n);
            return *this;
        }
        ManipulatorSelector& instance(const ::dds::core::InstanceHandle& h) {
            selector_.instance(h);
            return *this;
        }
        ManipulatorSelector& next_instance(const ::dds::core::InstanceHandle& h) {
            selector_.next_instance(h);
            return *this;
        }
        ManipulatorSelector& state(const ::dds::sub::status::DataState& s) {
            selector_.state(s);
            return *this;
        }
        ManipulatorSelector& content(const ::dds::sub::Query& query) {
            selector_.content(query);
            return *this;
        }

        /// Reads when `read_mode` is set, takes otherwise.
        ManipulatorSelector& operator>>(::dds::sub::LoanedSamples<T>& samples) {
            samples = read_mode_ ? selector_.read() : selector_.take();
            return *this;
        }
        ManipulatorSelector& operator>>(ManipulatorSelector& (*manipulator)(ManipulatorSelector&)) {
            return manipulator(*this);
        }
        template <typename Functor>
        ManipulatorSelector operator>>(Functor f) {
            f(*this);
            return *this;
        }

    private:
        Selector selector_;
        bool read_mode_;
    };

    /// `dr >> samples` takes the samples (the selector's default).
    TDataReader& operator>>(::dds::sub::LoanedSamples<T>& ls) {
        ls = Selector(*this).take();
        return *this;
    }
    ManipulatorSelector operator>>(ManipulatorSelector& (*manipulator)(ManipulatorSelector&)) {
        ManipulatorSelector selector(*this);
        manipulator(selector);
        return selector;
    }
    template <typename Functor>
    ManipulatorSelector operator>>(Functor f) {
        ManipulatorSelector selector(*this);
        f(selector);
        return selector;
    }

    /// `take` — removes the samples received so far that the default filter
    /// state selects.
    ::dds::sub::LoanedSamples<T> take() { return Selector(*this).take(); }
    /// `read` — non-destructive: DDS 1.2 §7.1.2.5.3.8 sets each sample's
    /// sample_state to READ and leaves it in the DataReader so a following
    /// read or take still finds it.
    ::dds::sub::LoanedSamples<T> read() { return Selector(*this).read(); }

    // Copy forms.
    template <typename SamplesFWIterator>
    uint32_t read(SamplesFWIterator sfit, uint32_t max_samples) {
        return Selector(*this).read(sfit, max_samples);
    }
    template <typename SamplesFWIterator>
    uint32_t take(SamplesFWIterator sfit, uint32_t max_samples) {
        return Selector(*this).take(sfit, max_samples);
    }
    template <typename SamplesBIIterator>
    uint32_t read(SamplesBIIterator sbit) {
        return Selector(*this).read(sbit);
    }
    template <typename SamplesBIIterator>
    uint32_t take(SamplesBIIterator sbit) {
        return Selector(*this).take(sbit);
    }

    /// `dr.select().instance(h).state(s).take()`.
    Selector select() { return Selector(*this); }

    /// Waits until the historical data of the matched writers arrived.
    void wait_for_historical_data(const ::dds::core::Duration& timeout) {
        this->delegate()->wait_for_historical_data(timeout);
    }

    // ---- Statuses ----------------------------------------------------------

    const ::dds::core::status::LivelinessChangedStatus& liveliness_changed_status() const {
        return this->delegate()->liveliness_changed_status();
    }
    const ::dds::core::status::SampleRejectedStatus& sample_rejected_status() const {
        return this->delegate()->sample_rejected_status();
    }
    const ::dds::core::status::SampleLostStatus& sample_lost_status() const {
        return this->delegate()->sample_lost_status();
    }
    const ::dds::core::status::RequestedDeadlineMissedStatus& requested_deadline_missed_status() {
        return this->delegate()->requested_deadline_missed_status();
    }
    const ::dds::core::status::RequestedIncompatibleQosStatus& requested_incompatible_qos_status()
        const {
        return this->delegate()->requested_incompatible_qos_status();
    }
    const ::dds::core::status::SubscriptionMatchedStatus& subscription_matched_status() const {
        return this->delegate()->subscription_matched_status();
    }

    // ---- Instances, QoS, navigation -----------------------------------------

    /// The sample of the first sample this reader returned of the instance
    /// `h`; `UnsupportedError` for an instance it returned none of.
    ::dds::topic::TopicInstance<T> key_value(const ::dds::core::InstanceHandle& h) {
        ::dds::topic::TopicInstance<T> i(h);
        this->delegate()->key_value(i.sample(), h);
        return i;
    }
    T& key_value(T& sample, const ::dds::core::InstanceHandle& h) {
        return this->delegate()->key_value(sample, h);
    }
    const ::dds::core::InstanceHandle lookup_instance(const T& key) const {
        return this->delegate()->lookup_instance(key);
    }

    /// The topic, content filtered topic or multi topic the reader reads.
    ::dds::topic::AnyTopicDescription topic_description() const {
        return this->delegate()->topic_description();
    }
    const ::dds::sub::Subscriber& subscriber() const { return this->delegate()->subscriber(); }

    const ::dds::sub::qos::DataReaderQos& qos() const { return this->delegate()->qos(); }
    void qos(const ::dds::sub::qos::DataReaderQos& the_qos) { this->delegate()->qos(the_qos); }
    TDataReader& operator<<(const ::dds::sub::qos::DataReaderQos& the_qos) {
        qos(the_qos);
        return *this;
    }
    TDataReader& operator>>(::dds::sub::qos::DataReaderQos& the_qos) {
        the_qos = qos();
        return *this;
    }

    /// Installs `l` for the statuses in `mask`; `nullptr` removes the listener.
    void listener(DataReaderListener<T>* l, const ::dds::core::status::StatusMask& mask) {
        if (!this->impl_) {
            throw ::dds::core::NullReferenceError("DataReader::listener: null reader");
        }
        this->delegate()->listener(l, mask);
        ::dds::core::detail::retain_for_listener(this->delegate(), l != nullptr);
    }
    DataReaderListener<T>* listener() const {
        return this->impl_ ? this->delegate()->listener() : nullptr;
    }


};

} // namespace detail

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_TDATAREADER_HPP
