// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/DataWriter.hpp — DataWriter<T> (DDS-PSM-Cxx 1.0 §7.9).

#ifndef ZERODDS_DDS_PUB_DATAWRITER_HPP
#define ZERODDS_DDS_PUB_DATAWRITER_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <utility>

#include "dds/core/Reference.hpp"
#include "dds/core/detail/Retain.hpp"
#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/Time.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/core/TEntity.hpp"
#include "dds/pub/DataWriterListener.hpp"
#include "dds/pub/detail/DataWriterDelegate.hpp"
#include "dds/pub/detail/DataWriterFwd.hpp"
#include "dds/pub/Publisher.hpp"
#include "dds/topic/Topic.hpp"
#include "dds/topic/TopicInstance.hpp"
#include "dds/core/detail/borrow.hpp"
#include "zerodds.h"

namespace dds {
namespace pub {

namespace detail {

/// What `dds::pub::find` needs of a writer: the `AnyDataWriter` of its delegate.
template <typename W, typename D>
struct WriterToAny {
    AnyDataWriter operator()(const ::zerodds::cxx::shared_ptr<void>& d) const {
        return AnyDataWriter(::dds::core::detail::wrap_delegate<W>(
            ::zerodds::cxx::static_pointer_cast<D>(d)));
    }
};

/// The C handle of the delegate of a writer, for comparison with the handles the C API returns.
template <typename W, typename D>
struct WriterNativeHandle {
    const void* operator()(const ::zerodds::cxx::shared_ptr<void>& d) const {
        return static_cast<D*>(d.get())->native_handle();
    }
};

} // namespace detail

/// DataWriter<T> (Spec §7.9). Reference type with delegate pattern
/// (DDS-PSM-Cxx 1.0 §7.5.1.1, §7.5.1.2).
template <typename T, template <typename Q> class DELEGATE>
class DataWriter : public ::dds::core::Reference<DELEGATE<T> > {
public:
    typedef DELEGATE<T> DELEGATE_T;
    typedef ::dds::core::Reference<DELEGATE_T> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;
    typedef DataWriterListener<T> Listener;

    /// Constructs a null writer.
protected:
    DataWriter() ZERODDS_NOEXCEPT {}
public:

    /// A writer over a reference to its delegate (`WeakReference<DataWriter<T> >::lock()`).
    explicit DataWriter(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// A writer of `topic` in `pub` with the default QoS.
    DataWriter(const Publisher& pub, const ::dds::topic::Topic<T>& topic)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE_T>(pub, topic)) {
        track(pub, topic.name());
    }

    /// With QoS, and optionally a listener.
    DataWriter(const Publisher& pub, const ::dds::topic::Topic<T>& topic,
               const ::dds::pub::qos::DataWriterQos& qos, DataWriterListener<T>* listener = 0,
               const ::dds::core::status::StatusMask& mask = ::dds::core::status::StatusMask::all())
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE_T>(pub, topic, qos)) {
        track(pub, topic.name());
        this->listener(listener, mask);
    }

    // Reference semantics: copy/assign share the delegate.
    /// The null DataWriter (`dds::core::null`, DDS-PSM-Cxx 1.0 §7.5.1.2).
    DataWriter(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    DataWriter(const DataWriter&) = default;
    DataWriter& operator=(const DataWriter&) = default;
    DataWriter(DataWriter&&) = default;
    DataWriter& operator=(DataWriter&&) = default;
    ~DataWriter() = default;
#endif

    // ---- Writing ---------------------------------------------------------

    void write(const T& sample) { this->delegate()->write(sample, nil(), 0); }
    void write(const T& sample, const ::dds::core::Time& timestamp) {
        this->delegate()->write(sample, nil(), &timestamp);
    }
    void write(const T& data, const ::dds::core::InstanceHandle& instance) {
        this->delegate()->write(data, instance, 0);
    }
    void write(const T& data, const ::dds::core::InstanceHandle& instance,
               const ::dds::core::Time& timestamp) {
        this->delegate()->write(data, instance, &timestamp);
    }
    void write(const ::dds::topic::TopicInstance<T>& i) { write(i.sample(), i.handle()); }
    void write(const ::dds::topic::TopicInstance<T>& i, const ::dds::core::Time& timestamp) {
        write(i.sample(), i.handle(), timestamp);
    }

    /// Writes the samples in `[begin, end)`.
    template <typename FWIterator>
    void write(const FWIterator& begin, const FWIterator& end) {
        for (FWIterator it = begin; it != end; ++it) {
            write(*it);
        }
    }
    template <typename FWIterator>
    void write(const FWIterator& begin, const FWIterator& end,
               const ::dds::core::Time& timestamp) {
        for (FWIterator it = begin; it != end; ++it) {
            write(*it, timestamp);
        }
    }
    /// Writes the samples in `[data_begin, data_end)` to the instances in
    /// `[handle_begin, handle_end)`, pairwise.
    template <typename SamplesFWIterator, typename HandlesFWIterator>
    void write(const SamplesFWIterator& data_begin, const SamplesFWIterator& data_end,
               const HandlesFWIterator& handle_begin, const HandlesFWIterator& handle_end) {
        HandlesFWIterator h = handle_begin;
        for (SamplesFWIterator it = data_begin; it != data_end; ++it) {
            if (h == handle_end) {
                throw ::dds::core::InvalidArgumentError(
                    "DataWriter::write: fewer instance handles than samples");
            }
            write(*it, *h);
            ++h;
        }
    }
    template <typename SamplesFWIterator, typename HandlesFWIterator>
    void write(const SamplesFWIterator& data_begin, const SamplesFWIterator& data_end,
               const HandlesFWIterator& handle_begin, const HandlesFWIterator& handle_end,
               const ::dds::core::Time& timestamp) {
        HandlesFWIterator h = handle_begin;
        for (SamplesFWIterator it = data_begin; it != data_end; ++it) {
            if (h == handle_end) {
                throw ::dds::core::InvalidArgumentError(
                    "DataWriter::write: fewer instance handles than samples");
            }
            write(*it, *h, timestamp);
            ++h;
        }
    }

    DataWriter& operator<<(const T& data) {
        write(data);
        return *this;
    }
    DataWriter& operator<<(const std::pair<T, ::dds::core::Time>& data) {
        write(data.first, data.second);
        return *this;
    }
    DataWriter& operator<<(const std::pair<T, ::dds::core::InstanceHandle>& data) {
        write(data.first, data.second);
        return *this;
    }
    DataWriter& operator<<(DataWriter& (*manipulator)(DataWriter&)) { return manipulator(*this); }

    // ---- Instances --------------------------------------------------------

    const ::dds::core::InstanceHandle register_instance(const T& key) {
        return this->delegate()->register_instance(key);
    }
    const ::dds::core::InstanceHandle register_instance(const T& key,
                                                         const ::dds::core::Time& ts) {
        return this->delegate()->register_instance(key, ts);
    }
    DataWriter& unregister_instance(const ::dds::core::InstanceHandle& i) {
        this->delegate()->unregister_instance(i, 0);
        return *this;
    }
    DataWriter& unregister_instance(const ::dds::core::InstanceHandle& i,
                                    const ::dds::core::Time& ts) {
        this->delegate()->unregister_instance(i, &ts);
        return *this;
    }
    DataWriter& dispose_instance(const ::dds::core::InstanceHandle& i) {
        this->delegate()->dispose_instance(i, 0);
        return *this;
    }
    DataWriter& dispose_instance(const ::dds::core::InstanceHandle& i,
                                 const ::dds::core::Time& ts) {
        this->delegate()->dispose_instance(i, &ts);
        return *this;
    }
    /// The key of the instance `h`, for an instance registered or looked up
    /// through this writer; `UnsupportedError` for another one.
    ::dds::topic::TopicInstance<T>& key_value(::dds::topic::TopicInstance<T>& i,
                                              const ::dds::core::InstanceHandle& h) {
        this->delegate()->key_value(i.sample(), h);
        i.handle(h);
        return i;
    }
    T& key_value(T& sample, const ::dds::core::InstanceHandle& h) {
        return this->delegate()->key_value(sample, h);
    }
    ::dds::core::InstanceHandle lookup_instance(const T& key) {
        return this->delegate()->lookup_instance(key);
    }

    // ---- QoS, navigation ---------------------------------------------------

    const ::dds::pub::qos::DataWriterQos& qos() const { return this->delegate()->qos(); }
    void qos(const ::dds::pub::qos::DataWriterQos& the_qos) { this->delegate()->qos(the_qos); }
    DataWriter& operator<<(const ::dds::pub::qos::DataWriterQos& the_qos) {
        qos(the_qos);
        return *this;
    }
    const DataWriter& operator>>(::dds::pub::qos::DataWriterQos& the_qos) const {
        the_qos = qos();
        return *this;
    }
    const ::dds::topic::Topic<T>& topic() const { return this->delegate()->topic(); }
    const ::dds::pub::Publisher& publisher() const { return this->delegate()->publisher(); }

    void wait_for_acknowledgments(const ::dds::core::Duration& timeout) {
        this->delegate()->wait_for_acknowledgments(timeout);
    }

    /// Installs `l` for the statuses in `mask`; `nullptr` removes the listener.
    void listener(DataWriterListener<T>* l, const ::dds::core::status::StatusMask& mask) {
        if (!this->impl_) {
            throw ::dds::core::NullReferenceError("DataWriter::listener: null writer");
        }
        this->delegate()->set_listener(l);
        if (l == 0) {
            this->delegate()->detach_listener();
            ::dds::core::detail::retain_for_listener(this->delegate(), false);
            return;
        }
        this->delegate()->attach_listener(detail::DataWriterListenerBridge<T>::table(l),
                                          static_cast<uint32_t>(mask.to_ulong()));
        ::dds::core::detail::retain_for_listener(this->delegate(), true);
    }
    DataWriterListener<T>* listener() const {
        return this->impl_ ? this->delegate()->listener() : 0;
    }

    // ---- Statuses ---------------------------------------------------------

    const ::dds::core::status::LivelinessLostStatus liveliness_lost_status() {
        return this->delegate()->liveliness_lost_status();
    }
    const ::dds::core::status::OfferedDeadlineMissedStatus offered_deadline_missed_status() {
        return this->delegate()->offered_deadline_missed_status();
    }
    const ::dds::core::status::OfferedIncompatibleQosStatus offered_incompatible_qos_status() {
        return this->delegate()->offered_incompatible_qos_status();
    }

    /// The names the normative header of DDS-PSM-Cxx 1.0 gives these two operations
    /// (dds/pub/DataWriter.hpp, which misspells them); the same calls as the two above.
    const ::dds::core::status::OfferedDeadlineMissedStatus offered_deadlined_missed_status() {
        return offered_deadline_missed_status();
    }
    const ::dds::core::status::OfferedIncompatibleQosStatus offered_incompatible_qos_staus() {
        return offered_incompatible_qos_status();
    }
    const ::dds::core::status::PublicationMatchedStatus publication_matched_status() {
        return this->delegate()->publication_matched_status();
    }
    void assert_liveliness() { this->delegate()->assert_liveliness(); }

    // TEntity operations.
    void enable() { this->delegate()->enable(); }
    ::dds::core::status::StatusMask status_changes() const {
        return this->delegate()->status_changes();
    }
    ::dds::core::InstanceHandle instance_handle() const {
        return this->delegate()->instance_handle();
    }
    void close() {
        const DELEGATE_REF_T keep = this->delegate();
        ::dds::core::detail::release_entity(keep);
        keep->close();
    }
    void retain() {
        this->delegate()->retain();
        ::dds::core::detail::retain_entity(this->delegate());
    }

private:
    /// Registers the writer with its publisher, where `dds::pub::find` finds it.
    void track(const Publisher& pub, const std::string& topic_name) {
        typedef ::dds::core::detail::EntityTracker<AnyDataWriter> Tracker;
        typename Tracker::Entry e = {
            topic_name, ::zerodds::cxx::type_index(typeid(T)),
            ::zerodds::cxx::weak_ptr<void>(this->impl_),
            detail::WriterToAny<DataWriter<T>, DELEGATE_T>(),
            detail::WriterNativeHandle<DataWriter<T>, DELEGATE_T>()};
        pub->writers().add(ZERODDS_MOVE(e));
    }

    static const ::dds::core::InstanceHandle& nil() {
        static const ::dds::core::InstanceHandle handle;
        return handle;
    }
};

} // namespace pub
} // namespace dds

// AnyDataWriter holds a DataWriter<T> of any T.
#include "dds/pub/detail/AnyDataWriterDetail.hpp"

#endif // ZERODDS_DDS_PUB_DATAWRITER_HPP
