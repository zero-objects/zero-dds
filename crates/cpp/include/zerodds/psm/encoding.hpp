// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds/psm/encoding.hpp — `::zerodds::wire::Encoding`, the one name of the wire runtime that
// the headers of `dds/` use (the XCDR version a sample was written with). With a C++11 compiler it
// is the type of `zerodds/wire.hpp`; with a C++03 compiler, which cannot compile that runtime, a
// plain enumeration with the same enumerators.

#ifndef ZERODDS_PSM_ENCODING_HPP
#define ZERODDS_PSM_ENCODING_HPP

#include "zerodds/psm/cxx.hpp"

#if ZERODDS_CXX11
#include "zerodds/wire.hpp"
#else
namespace zerodds {
namespace wire {

struct Encoding {
    enum Value { PlainCdr1, PlCdr1, PlainCdr2, DelimitedCdr, PlCdr2 };
    Encoding(Value v = PlainCdr1) : value_(v) {}
    operator Value() const { return value_; }

private:
    Value value_;
};

}  // namespace wire
}  // namespace zerodds
#endif

#endif  // ZERODDS_PSM_ENCODING_HPP
