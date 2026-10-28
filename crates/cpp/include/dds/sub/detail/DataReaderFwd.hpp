// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/detail/DataReaderFwd.hpp — declaration of `dds::sub::DataReader` with its default
// delegate: `template <typename T, template <typename Q> class DELEGATE = detail::DataReader>
// class DataReader` of DDS-PSM-Cxx 1.0 (dds/sub/DataReader.hpp).

#ifndef ZERODDS_DDS_SUB_DETAIL_DATAREADERFWD_HPP
#define ZERODDS_DDS_SUB_DETAIL_DATAREADERFWD_HPP

namespace dds {
namespace sub {
namespace detail {
template <typename T>
class DataReaderDelegate;
} // namespace detail

template <typename T, template <typename Q> class DELEGATE = detail::DataReaderDelegate>
class DataReader;

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_DETAIL_DATAREADERFWD_HPP
