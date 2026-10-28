// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds/psm/expression_parameters.hpp — the filter expression and its
// parameters that dds::topic::Filter (DDS-PSM-Cxx 1.0 §7.8) and
// dds::sub::Query (§7.13.2) both hold, with the accessors both offer.
// Vendor helper, kept out of `dds::core` (§2.2).

#ifndef ZERODDS_PSM_EXPRESSION_PARAMETERS_HPP
#define ZERODDS_PSM_EXPRESSION_PARAMETERS_HPP

#include "zerodds/psm/cxx.hpp"
#include <stdint.h>
#include <string>
#include <vector>

namespace zerodds {
namespace psm {

class ExpressionParameters {
public:
    typedef std::vector<std::string>::iterator iterator;
    typedef std::vector<std::string>::const_iterator const_iterator;

    explicit ExpressionParameters(const std::string& expression) : expression_(expression) {}
    template <typename FWIterator>
    ExpressionParameters(const std::string& expression, const FWIterator& params_begin,
                         const FWIterator& params_end)
        : expression_(expression), parameters_(params_begin, params_end) {}
    ExpressionParameters(const std::string& expression, const std::vector<std::string>& params)
        : expression_(expression), parameters_(params) {}

    const std::string& expression() const { return expression_; }
    void expression(const std::string& expr) { expression_ = expr; }

    const_iterator begin() const { return parameters_.begin(); }
    const_iterator end() const { return parameters_.end(); }
    iterator begin() { return parameters_.begin(); }
    iterator end() { return parameters_.end(); }

    template <typename FWIterator>
    void parameters(const FWIterator& begin, const FWIterator end) {
        parameters_.assign(begin, end);
    }
    void add_parameter(const std::string& param) { parameters_.push_back(param); }
    uint32_t parameters_length() const { return static_cast<uint32_t>(parameters_.size()); }

private:
    std::string expression_;
    std::vector<std::string> parameters_;
};

} // namespace psm
} // namespace zerodds

#endif // ZERODDS_PSM_EXPRESSION_PARAMETERS_HPP
