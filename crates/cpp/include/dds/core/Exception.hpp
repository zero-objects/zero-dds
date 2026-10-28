// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/Exception.hpp — DDS-PSM-Cxx 1.0 §7.5.5 exception hierarchy.

#ifndef ZERODDS_DDS_CORE_EXCEPTION_HPP
#define ZERODDS_DDS_CORE_EXCEPTION_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdexcept>
#include <string>

namespace dds {
namespace core {

/// Base exception (Spec §7.5.5). All DDS exceptions derive from this.
///
/// The classes below inherit it virtually: a class that is also another
/// library's exception of the same name (DDS-RPC 1.0 §7.11.1.3, whose local
/// errors are these classes) has the one `Exception` subobject, and
/// `catch (const dds::core::Exception&)` reaches it unambiguously.
class Exception {
protected:
    Exception() {}
public:
    virtual ~Exception() ZERODDS_NOEXCEPT {}
    virtual const char* what() const ZERODDS_NOEXCEPT = 0;
};

/// Generic DDS error (Spec §7.5.5 Table 7.3: ERROR -> std::logic_error).
class Error : public virtual Exception, public std::exception {
public:
    Error() ZERODDS_NOEXCEPT {}
    explicit Error(const std::string& msg) : msg_(msg) {}
    virtual ~Error() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return msg_.c_str(); }
private:
    std::string msg_;
};

/// Operation against a deleted entity (ALREADY_DELETED -> std::logic_error).
class AlreadyClosedError : public virtual Exception, public std::logic_error {
public:
    explicit AlreadyClosedError(const std::string& msg) : std::logic_error(msg) {}
    AlreadyClosedError(const AlreadyClosedError& src) : std::logic_error(src.what()) {}
    virtual ~AlreadyClosedError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::logic_error::what(); }
};

/// Argument check failed (ILLEGAL_OPERATION -> std::logic_error).
class IllegalOperationError : public virtual Exception, public std::logic_error {
public:
    explicit IllegalOperationError(const std::string& msg) : std::logic_error(msg) {}
    IllegalOperationError(const IllegalOperationError& src) : std::logic_error(src.what()) {}
    virtual ~IllegalOperationError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::logic_error::what(); }
};

/// Immutable QoS-policy modified (IMMUTABLE_POLICY -> std::logic_error).
class ImmutablePolicyError : public virtual Exception, public std::logic_error {
public:
    explicit ImmutablePolicyError(const std::string& msg) : std::logic_error(msg) {}
    ImmutablePolicyError(const ImmutablePolicyError& src) : std::logic_error(src.what()) {}
    virtual ~ImmutablePolicyError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::logic_error::what(); }
};

/// Inconsistent QoS-policies (INCONSISTENT_POLICY -> std::logic_error).
class InconsistentPolicyError : public virtual Exception, public std::logic_error {
public:
    explicit InconsistentPolicyError(const std::string& msg) : std::logic_error(msg) {}
    InconsistentPolicyError(const InconsistentPolicyError& src) : std::logic_error(src.what()) {}
    virtual ~InconsistentPolicyError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::logic_error::what(); }
};

/// Bad parameter (BAD_PARAMETER -> std::invalid_argument).
class InvalidArgumentError : public virtual Exception, public std::invalid_argument {
public:
    explicit InvalidArgumentError(const std::string& msg) : std::invalid_argument(msg) {}
    InvalidArgumentError(const InvalidArgumentError& src) : std::invalid_argument(src.what()) {}
    virtual ~InvalidArgumentError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::invalid_argument::what(); }
};

/// Entity not enabled (NOT_ENABLED -> std::logic_error).
class NotEnabledError : public virtual Exception, public std::logic_error {
public:
    explicit NotEnabledError(const std::string& msg) : std::logic_error(msg) {}
    NotEnabledError(const NotEnabledError& src) : std::logic_error(src.what()) {}
    virtual ~NotEnabledError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::logic_error::what(); }
};

/// Resource limit reached (OUT_OF_RESOURCES -> std::runtime_error).
class OutOfResourcesError : public virtual Exception, public std::runtime_error {
public:
    explicit OutOfResourcesError(const std::string& msg) : std::runtime_error(msg) {}
    OutOfResourcesError(const OutOfResourcesError& src) : std::runtime_error(src.what()) {}
    virtual ~OutOfResourcesError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::runtime_error::what(); }
};

/// Pre-condition violation (PRECONDITION_NOT_MET -> std::logic_error).
class PreconditionNotMetError : public virtual Exception, public std::logic_error {
public:
    explicit PreconditionNotMetError(const std::string& msg) : std::logic_error(msg) {}
    PreconditionNotMetError(const PreconditionNotMetError& src) : std::logic_error(src.what()) {}
    virtual ~PreconditionNotMetError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::logic_error::what(); }
};

/// Timeout reached (TIMEOUT -> std::runtime_error).
class TimeoutError : public virtual Exception, public std::runtime_error {
public:
    explicit TimeoutError(const std::string& msg) : std::runtime_error(msg) {}
    TimeoutError(const TimeoutError& src) : std::runtime_error(src.what()) {}
    virtual ~TimeoutError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::runtime_error::what(); }
};

/// Operation not supported in current configuration (UNSUPPORTED -> std::logic_error).
class UnsupportedError : public virtual Exception, public std::logic_error {
public:
    explicit UnsupportedError(const std::string& msg) : std::logic_error(msg) {}
    UnsupportedError(const UnsupportedError& src) : std::logic_error(src.what()) {}
    virtual ~UnsupportedError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::logic_error::what(); }
};

/// Invalid polymorphic downcast.
class InvalidDowncastError : public virtual Exception, public std::runtime_error {
public:
    explicit InvalidDowncastError(const std::string& msg) : std::runtime_error(msg) {}
    InvalidDowncastError(const InvalidDowncastError& src) : std::runtime_error(src.what()) {}
    virtual ~InvalidDowncastError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::runtime_error::what(); }
};

/// Null reference error.
class NullReferenceError : public virtual Exception, public std::runtime_error {
public:
    explicit NullReferenceError(const std::string& msg) : std::runtime_error(msg) {}
    NullReferenceError(const NullReferenceError& src) : std::runtime_error(src.what()) {}
    virtual ~NullReferenceError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::runtime_error::what(); }
};

/// Invalid data error.
class InvalidDataError : public virtual Exception, public std::logic_error {
public:
    explicit InvalidDataError(const std::string& msg) : std::logic_error(msg) {}
    InvalidDataError(const InvalidDataError& src) : std::logic_error(src.what()) {}
    virtual ~InvalidDataError() ZERODDS_NOEXCEPT {}
    const char* what() const ZERODDS_NOEXCEPT ZERODDS_OVERRIDE { return std::logic_error::what(); }
};


} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_EXCEPTION_HPP