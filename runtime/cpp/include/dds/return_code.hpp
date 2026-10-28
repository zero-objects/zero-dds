// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/return_code.hpp — DDS 1.4 §2.2.2.1.1, ReturnCode_t: the codes the operations of the
// generated classes answer with (the map operations of DDS-XTypes 1.3 §7.5.1.3.1, Table 47).
//
// Apart from `dds/types.hpp` because the wire runtime includes the map class, and the map class
// needs these, and `dds/types.hpp` needs the wire runtime.
#ifndef DDS_RETURN_CODE_HPP
#define DDS_RETURN_CODE_HPP

#include <stdint.h>

namespace DDS {

typedef int32_t ReturnCode_t;
const ReturnCode_t RETCODE_OK = 0;
const ReturnCode_t RETCODE_ERROR = 1;
const ReturnCode_t RETCODE_UNSUPPORTED = 2;
const ReturnCode_t RETCODE_BAD_PARAMETER = 3;
const ReturnCode_t RETCODE_PRECONDITION_NOT_MET = 4;
const ReturnCode_t RETCODE_OUT_OF_RESOURCES = 5;
const ReturnCode_t RETCODE_NOT_ENABLED = 6;
const ReturnCode_t RETCODE_IMMUTABLE_POLICY = 7;
const ReturnCode_t RETCODE_INCONSISTENT_POLICY = 8;
const ReturnCode_t RETCODE_ALREADY_DELETED = 9;
const ReturnCode_t RETCODE_TIMEOUT = 10;
const ReturnCode_t RETCODE_NO_DATA = 11;
const ReturnCode_t RETCODE_ILLEGAL_OPERATION = 12;

}  // namespace DDS

#endif  // DDS_RETURN_CODE_HPP
