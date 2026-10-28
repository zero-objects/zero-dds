// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/detail/AnyDataWriterDetail.hpp — the holders behind AnyDataWriter.
// Included where DataWriter and Publisher are complete.

#ifndef ZERODDS_DDS_PUB_DETAIL_ANYDATAWRITERDETAIL_HPP
#define ZERODDS_DDS_PUB_DETAIL_ANYDATAWRITERDETAIL_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include "dds/core/detail/borrow.hpp"
#include <string>

#include "dds/pub/AnyDataWriter.hpp"
#include "dds/pub/DataWriter.hpp"
#include "dds/pub/Publisher.hpp"

namespace dds {
namespace pub {
namespace detail {

template <typename T>
class DWHolder : public DWHolderBase {
public:
    explicit DWHolder(const DataWriter<T>& dw)
        : dw_(dw), topic_name_(dw.topic().name()), type_name_(dw.topic().type_name()) {}

    const ::dds::pub::qos::DataWriterQos& qos() const ZERODDS_OVERRIDE { return dw_.qos(); }
    void qos(const ::dds::pub::qos::DataWriterQos& q) ZERODDS_OVERRIDE { dw_.qos(q); }
    const std::string& topic_name() const ZERODDS_OVERRIDE { return topic_name_; }
    const std::string& type_name() const ZERODDS_OVERRIDE { return type_name_; }
    const ::dds::pub::Publisher& publisher() const ZERODDS_OVERRIDE { return dw_.publisher(); }
    void wait_for_acknowledgments(const ::dds::core::Duration& timeout) ZERODDS_OVERRIDE {
        dw_.wait_for_acknowledgments(timeout);
    }
    void close() ZERODDS_OVERRIDE { dw_.close(); }
    void retain(bool) ZERODDS_OVERRIDE { dw_.retain(); }

    DataWriter<T> writer() const { return dw_; }

private:
    DataWriter<T> dw_;
    std::string topic_name_;
    std::string type_name_;
};

class NativeDataWriterHolder : public DWHolderBase {
public:
    explicit NativeDataWriterHolder(zerodds_ZeroDdsDataWriter* raw)
        : raw_(raw), pub_(::dds::core::detail::borrow<Publisher>(zerodds_dw_get_publisher(raw))) {
        if (zerodds_ZeroDdsTopic* t = zerodds_dw_get_topic(raw)) {
            if (char* n = zerodds_topic_get_name(t)) {
                topic_name_ = n;
                zerodds_string_free(n);
            }
            if (char* n = zerodds_topic_get_type_name(t)) {
                type_name_ = n;
                zerodds_string_free(n);
            }
        }
    }

    const ::dds::pub::qos::DataWriterQos& qos() const ZERODDS_OVERRIDE {
        qos_ = ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_dw_get_qos, raw_),
        &::dds::core::detail::datawriter_qos_of, "AnyDataWriter::qos",
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsDataWriterQos>()(&zerodds_ZeroDdsDataWriterQos::user_data)(&zerodds_ZeroDdsDataWriterQos::topic_data)(&zerodds_ZeroDdsDataWriterQos::group_data));
        return qos_;
    }
    void qos(const ::dds::pub::qos::DataWriterQos& q) ZERODDS_OVERRIDE {
        zerodds_ZeroDdsDataWriterQos native = ::dds::core::detail::to_native(q);
        ::zerodds::psm::check_status(zerodds_dw_set_qos(raw_, &native), "AnyDataWriter::qos");
    }
    const std::string& topic_name() const ZERODDS_OVERRIDE { return topic_name_; }
    const std::string& type_name() const ZERODDS_OVERRIDE { return type_name_; }
    const ::dds::pub::Publisher& publisher() const ZERODDS_OVERRIDE { return pub_; }
    void wait_for_acknowledgments(const ::dds::core::Duration& timeout) ZERODDS_OVERRIDE {
        uint64_t ms = ::dds::core::detail::is_infinite(timeout) ? ZERODDS_BLOCKING_INFINITE
                                            : static_cast<uint64_t>(timeout.to_millisecs());
        int rc = zerodds_dw_wait_for_acknowledgments(raw_, ms);
        if (rc == -4) throw ::dds::core::TimeoutError("AnyDataWriter::wait_for_acknowledgments");
        ::zerodds::psm::check_status(rc, "AnyDataWriter::wait_for_acknowledgments");
    }
    void close() ZERODDS_OVERRIDE {
        throw ::dds::core::IllegalOperationError(
            "AnyDataWriter::close: the writer belongs to the listener caller");
    }
    void retain(bool) ZERODDS_OVERRIDE {}

    zerodds_ZeroDdsDataWriter* native() const { return raw_; }

private:
    zerodds_ZeroDdsDataWriter* raw_;
    ::dds::pub::Publisher pub_;
    std::string topic_name_;
    std::string type_name_;
    mutable ::dds::pub::qos::DataWriterQos qos_;
};

struct AnyDataWriterAccess {
    static AnyDataWriter make(::zerodds::cxx::shared_ptr<DWHolderBase> holder) {
        AnyDataWriter any;
        any.impl_ = ZERODDS_MOVE(holder);
        return any;
    }
};

inline AnyDataWriter make_native_any_writer(zerodds_ZeroDdsDataWriter* raw) {
    return AnyDataWriterAccess::make(::zerodds::cxx::make_shared<NativeDataWriterHolder>(raw));
}

} // namespace detail

template <typename T>
AnyDataWriter::AnyDataWriter(const DataWriter<T>& dw)
    : BASE_T(::zerodds::cxx::make_shared<detail::DWHolder<T> >(dw)) {}

template <typename T>
AnyDataWriter& AnyDataWriter::operator=(const DataWriter<T>& rhs) {
    this->impl_ = ::zerodds::cxx::make_shared<detail::DWHolder<T> >(rhs);
    return *this;
}

template <typename T>
DataWriter<T> AnyDataWriter::get() {
    detail::DWHolderBase& h = holder();
    if (detail::DWHolder<T>* typed = dynamic_cast<detail::DWHolder<T>*>(&h)) {
        return typed->writer();
    }
    if (detail::NativeDataWriterHolder* native = dynamic_cast<detail::NativeDataWriterHolder*>(&h)) {
        if (native->type_name() == ::dds::topic::detail::get_type_name<T>()) {
            return ::dds::core::detail::borrow<DataWriter<T> >(native->native());
        }
    }
    throw ::dds::core::InvalidDowncastError("AnyDataWriter::get: the writer is of another type");
}

} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_DETAIL_ANYDATAWRITERDETAIL_HPP
