// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/Query.hpp — Query (DDS-PSM-Cxx 1.0 §7.13.2): a content filter
// expression with its parameters, for a reader's `select().content(query)`
// and for a QueryCondition.

#ifndef ZERODDS_DDS_SUB_QUERY_HPP
#define ZERODDS_DDS_SUB_QUERY_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <iterator>
#include <string>
#include <vector>

#include "dds/sub/detail/DataReaderFwd.hpp"
#include "dds/sub/AnyDataReader.hpp"
#include "zerodds/psm/expression_parameters.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {


class Query : public ::zerodds::psm::ExpressionParameters {
public:
    template <typename T>
    Query(const DataReader<T>& dr, const std::string& query_expression)
        : ::zerodds::psm::ExpressionParameters(query_expression), reader_(dr) {}

    template <typename T, typename FWIterator>
    Query(const DataReader<T>& dr, const std::string& query_expression,
          const FWIterator& params_begin, const FWIterator& params_end)
        : ::zerodds::psm::ExpressionParameters(query_expression, params_begin, params_end),
          reader_(dr) {}

    template <typename T>
    Query(const DataReader<T>& dr, const std::string& query_expression,
          const std::vector<std::string>& params)
        : ::zerodds::psm::ExpressionParameters(query_expression, params), reader_(dr) {}

    /// The reader the query was made for.
    const AnyDataReader& data_reader() const { return reader_; }

private:
    AnyDataReader reader_;
};

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_QUERY_HPP
