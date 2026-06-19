// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/Exception.hpp — DDS-PSM-Cxx 1.0 §7.5.1 exception hierarchy.

#ifndef ZERODDS_DDS_CORE_EXCEPTION_HPP
#define ZERODDS_DDS_CORE_EXCEPTION_HPP

#include <stdexcept>
#include <string>

namespace dds {
namespace core {

/// Base exception (Spec §7.5.1).
class Exception : public std::runtime_error {
public:
    explicit Exception(const std::string& what) : std::runtime_error(what) {}
};

/// Operation against a deleted entity.
class AlreadyClosedError : public Exception {
public:
    explicit AlreadyClosedError(const std::string& w) : Exception(w) {}
};

/// Generic Error.
class Error : public Exception {
public:
    explicit Error(const std::string& w) : Exception(w) {}
};

/// Argument check failed.
class IllegalOperationError : public Exception {
public:
    explicit IllegalOperationError(const std::string& w) : Exception(w) {}
};

/// Resource limit reached.
class OutOfResourcesError : public Exception {
public:
    explicit OutOfResourcesError(const std::string& w) : Exception(w) {}
};

/// Operation not supported in current configuration.
class UnsupportedError : public Exception {
public:
    explicit UnsupportedError(const std::string& w) : Exception(w) {}
};

/// Pre-condition violation.
class PreconditionNotMetError : public Exception {
public:
    explicit PreconditionNotMetError(const std::string& w) : Exception(w) {}
};

/// Bad parameter.
class InvalidArgumentError : public Exception {
public:
    explicit InvalidArgumentError(const std::string& w) : Exception(w) {}
};

/// Immutable QoS-policy modified.
class ImmutablePolicyError : public Exception {
public:
    explicit ImmutablePolicyError(const std::string& w) : Exception(w) {}
};

/// Inconsistent QoS-policies.
class InconsistentPolicyError : public Exception {
public:
    explicit InconsistentPolicyError(const std::string& w) : Exception(w) {}
};

/// Timeout reached.
class TimeoutError : public Exception {
public:
    explicit TimeoutError(const std::string& w) : Exception(w) {}
};

/// Entity not enabled.
class NotEnabledError : public Exception {
public:
    explicit NotEnabledError(const std::string& w) : Exception(w) {}
};

/// Throws the right `dds::core::*` exception based on the C-FFI status code.
inline void check_status(int code, const char* what) {
    switch (code) {
        case 0: return; // ZERODDS_OK
        case -1: throw Error(what);
        case -2: throw AlreadyClosedError(what);
        case -3: throw InvalidArgumentError(what);
        case -4: throw TimeoutError(what);
        case -5: throw PreconditionNotMetError(what);
        case -6: throw InvalidArgumentError(what);
        case -7: return; // NO_DATA — not an exception
        case -8: throw OutOfResourcesError(what);
        case -9: throw NotEnabledError(what);
        case -10: throw ImmutablePolicyError(what);
        case -11: throw InconsistentPolicyError(what);
        case -12: throw AlreadyClosedError(what);
        case -13: throw UnsupportedError(what);
        case -14: throw IllegalOperationError(what);
        default: throw Error(what);
    }
}

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_EXCEPTION_HPP
