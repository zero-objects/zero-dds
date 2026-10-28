// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/detail/AnyDataReaderDetail.hpp — the holders behind AnyDataReader.
// Included where DataReader and Subscriber are complete.

#ifndef ZERODDS_DDS_SUB_DETAIL_ANYDATAREADERDETAIL_HPP
#define ZERODDS_DDS_SUB_DETAIL_ANYDATAREADERDETAIL_HPP


#include "zerodds/psm/cxx.hpp"
#include "zerodds/psm/check_status.hpp"
#include "dds/core/detail/borrow.hpp"
#include <memory>
#include <string>

#include "dds/sub/AnyDataReader.hpp"
#include "dds/sub/DataReader.hpp"
#include "dds/sub/detail/QosFetch.hpp"
#include "dds/sub/Subscriber.hpp"

namespace dds {
namespace sub {
namespace detail {

template <typename T>
class DRHolder : public DRHolderBase {
public:
    explicit DRHolder(const DataReader<T>& dr)
        : dr_(dr),
          topic_name_(dr.topic_description().name()),
          type_name_(dr.topic_description().type_name()) {}

    const ::dds::sub::qos::DataReaderQos& qos() const ZERODDS_OVERRIDE { return dr_.qos(); }
    void qos(const ::dds::sub::qos::DataReaderQos& q) ZERODDS_OVERRIDE { dr_.qos(q); }
    const std::string& topic_name() const ZERODDS_OVERRIDE { return topic_name_; }
    const std::string& type_name() const ZERODDS_OVERRIDE { return type_name_; }
    const ::dds::sub::Subscriber& subscriber() const ZERODDS_OVERRIDE { return dr_.subscriber(); }
    void wait_for_historical_data(const ::dds::core::Duration& timeout) ZERODDS_OVERRIDE {
        dr_.wait_for_historical_data(timeout);
    }
    void close() ZERODDS_OVERRIDE { dr_.close(); }
    void retain(bool) ZERODDS_OVERRIDE { dr_.retain(); }
    zerodds_ZeroDdsDataReader* native_reader() const ZERODDS_OVERRIDE { return dr_->native_handle(); }

    DataReader<T> reader() const { return dr_; }

private:
    DataReader<T> dr_;
    std::string topic_name_;
    std::string type_name_;
};

class NativeDataReaderHolder : public DRHolderBase {
public:
    explicit NativeDataReaderHolder(zerodds_ZeroDdsDataReader* raw)
        : raw_(raw), sub_(::dds::core::detail::borrow<Subscriber>(zerodds_dr_get_subscriber(raw))) {
        if (zerodds_ZeroDdsTopic* t = zerodds_dr_get_topicdescription(raw)) {
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

    const ::dds::sub::qos::DataReaderQos& qos() const ZERODDS_OVERRIDE {
        qos_ = fetch_datareader_qos(raw_, "AnyDataReader::qos");
        return qos_;
    }
    void qos(const ::dds::sub::qos::DataReaderQos& q) ZERODDS_OVERRIDE {
        zerodds_ZeroDdsDataReaderQos native = ::dds::core::detail::to_native(q);
        ::zerodds::psm::check_status(zerodds_dr_set_qos(raw_, &native), "AnyDataReader::qos");
    }
    const std::string& topic_name() const ZERODDS_OVERRIDE { return topic_name_; }
    const std::string& type_name() const ZERODDS_OVERRIDE { return type_name_; }
    const ::dds::sub::Subscriber& subscriber() const ZERODDS_OVERRIDE { return sub_; }
    void wait_for_historical_data(const ::dds::core::Duration& timeout) ZERODDS_OVERRIDE {
        int rc = zerodds_dr_wait_for_historical_data(
            raw_, ::dds::core::detail::is_infinite(timeout) ? 0x7fffffff : timeout.sec(),
            ::dds::core::detail::is_infinite(timeout) ? 999999999u : timeout.nanosec());
        if (rc == -4) throw ::dds::core::TimeoutError("AnyDataReader::wait_for_historical_data");
        ::zerodds::psm::check_status(rc, "AnyDataReader::wait_for_historical_data");
    }
    void close() ZERODDS_OVERRIDE {
        throw ::dds::core::IllegalOperationError(
            "AnyDataReader::close: the reader belongs to the listener caller");
    }
    void retain(bool) ZERODDS_OVERRIDE {}
    zerodds_ZeroDdsDataReader* native_reader() const ZERODDS_OVERRIDE { return raw_; }

    zerodds_ZeroDdsDataReader* native() const { return raw_; }

private:
    zerodds_ZeroDdsDataReader* raw_;
    ::dds::sub::Subscriber sub_;
    std::string topic_name_;
    std::string type_name_;
    mutable ::dds::sub::qos::DataReaderQos qos_;
};

struct AnyDataReaderAccess {
    static AnyDataReader make(::zerodds::cxx::shared_ptr<DRHolderBase> holder) {
        AnyDataReader any;
        any.impl_ = ZERODDS_MOVE(holder);
        return any;
    }
};

inline AnyDataReader make_native_any_reader(zerodds_ZeroDdsDataReader* raw) {
    return AnyDataReaderAccess::make(::zerodds::cxx::make_shared<NativeDataReaderHolder>(raw));
}

} // namespace detail

template <typename T>
AnyDataReader::AnyDataReader(const DataReader<T>& dr)
    : BASE_T(::zerodds::cxx::make_shared<detail::DRHolder<T> >(dr)) {}

template <typename T>
AnyDataReader& AnyDataReader::operator=(const DataReader<T>& rhs) {
    this->impl_ = ::zerodds::cxx::make_shared<detail::DRHolder<T> >(rhs);
    return *this;
}

template <typename T>
DataReader<T> AnyDataReader::get() {
    detail::DRHolderBase& h = holder();
    if (detail::DRHolder<T>* typed = dynamic_cast<detail::DRHolder<T>*>(&h)) {
        return typed->reader();
    }
    if (detail::NativeDataReaderHolder* native = dynamic_cast<detail::NativeDataReaderHolder*>(&h)) {
        if (native->type_name() == ::dds::topic::detail::get_type_name<T>()) {
            return ::dds::core::detail::borrow<DataReader<T> >(native->native());
        }
    }
    throw ::dds::core::InvalidDowncastError("AnyDataReader::get: the reader is of another type");
}

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_DETAIL_ANYDATAREADERDETAIL_HPP
