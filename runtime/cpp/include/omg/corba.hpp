// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// omg/corba.hpp: the CORBA platform of IDL4 to C++ Language Mapping 1.0
// Annex A.1, for the code `zerodds-idlc generate --lang c++ --cpp-mapping
// idl4 --cpp-platform corba` writes. Header only, C++11.
//
// - CORBA::traits and CORBA::servant_traits (A.1.1), CORBA::make_reference.
// - The exception hierarchy of A.1.2: Exception, UserException,
//   SystemException with every standard system exception, and
//   UnknownUserException. A bound an omg::types bounded type exceeds is
//   CORBA::BAD_PARAM (A.1.2.4).
// - CORBA::TypeCode (A.1.3) with the creation operations of CORBA 3.3
//   part 1 §8.11.3 and the _tc_ constants of the basic types.
// - CORBA::Any (A.1.7): insertion and extraction by C++ type, the TypeCode
//   of the value, and its replacement by an equivalent one (A.1.7.4).
// - CORBA::Object (A.1.5), LocalObject (A.1.6), AbstractBase (A.1.9.1),
//   ValueBase (A.1.8.5) and ValueFactoryBase (A.1.8.9.1).
//
// An ORB and its object adapter are the subject of omg/corba_orb.hpp.
//
// Include this header, not omg/types.hpp, ahead of the generated code:
// it routes the bound check of omg/types.hpp to CORBA::BAD_PARAM.

#ifndef OMG_CORBA_HPP
#define OMG_CORBA_HPP

#include <cstddef>
#include <cstdint>
#include <exception>
#include <memory>
#include <set>
#include <string>
#include <type_traits>
#include <typeinfo>
#include <utility>
#include <vector>

namespace CORBA {
namespace detail {
// A.1.2.4: "the mapped C++ entity shall throw a CORBA::BAD_PARAM system
// exception when the application detects that specified bounds have been
// exceeded". Defined below, after BAD_PARAM.
[[noreturn]] inline void bound_exceeded(const char* what, std::size_t size, std::size_t bound);
// A.1.5.3: "Any attempt to invoke an operation through a nil object
// reference shall result in an INV_OBJREF exception." Defined below, after
// INV_OBJREF.
[[noreturn]] inline void nil_reference();
}  // namespace detail
}  // namespace CORBA

#if defined(OMG_TYPES_HPP) && !defined(OMG_CORBA_BOUND_HOOK)
#error "omg/corba.hpp has to be included before omg/types.hpp (A.1.2.4)"
#endif
#define OMG_CORBA_BOUND_HOOK 1
#define OMG_TYPES_BOUND_EXCEEDED(what, size, bound) ::CORBA::detail::bound_exceeded(what, size, bound)
#define OMG_TYPES_NIL_REFERENCE() ::CORBA::detail::nil_reference()
#include <omg/types.hpp>

namespace CORBA {

class Any;
class TypeCode;
class Object;
class ValueBase;
class AbstractBase;
class ORB;
class InterfaceDef;
class Policy;
class DomainManager;
class Context;
class NVList;
class Request;
struct NamedValue;
enum class SetOverrideType;

// -- A.1.1: traits ------------------------------------------------------------

// "For any IDL defined type a CORBA type trait shall be provided": the
// specializations for the basic types, strings, sequences, arrays and maps
// are below; the generated code specializes it for what it declares.
template <typename T, typename Enable = void>
struct traits;

// A.1.1.11 and A.1.10.3: the servant traits of an interface; generated.
template <typename T>
struct servant_traits;

template <typename T>
using servant_reference = typename servant_traits<T>::ref_type;
template <typename T>
using weak_servant_reference = typename servant_traits<T>::weak_ref_type;

// A.1.1.10: "CORBA::make_reference<>(...) template which shall deliver
// std::make_shared semantics".
namespace detail {
// What makes and destroys the instances of a class whose constructors and
// destructor are protected, as A.1.8.6 has them for a value box: the
// generated class names this struct a friend.
struct reference_access {
    template <typename T, typename... Args>
    static omg::types::ref_type<T> make(std::true_type, Args&&... args) {
        return std::make_shared<T>(std::forward<Args>(args)...);
    }
    template <typename T, typename... Args>
    static omg::types::ref_type<T> make(std::false_type, Args&&... args) {
        return std::shared_ptr<T>(new T(std::forward<Args>(args)...), [](T* instance) { delete instance; });
    }
};
}  // namespace detail

template <typename T, typename... Args>
inline omg::types::ref_type<T> make_reference(Args&&... args) {
    return detail::reference_access::make<T>(
        std::integral_constant<bool, std::is_constructible<T, Args&&...>::value>(), std::forward<Args>(args)...);
}

namespace detail {
// Table A.1 with Tables A.13 and A.14: a basic type, an enumeration, a
// reference type by value as `in`; a structured type by `const T&`.
template <typename T, bool by_value>
struct common_traits {
    using value_type = T;
    using in_type = typename std::conditional<by_value, T, const T&>::type;
    using out_type = T&;
    using inout_type = T&;
};
}  // namespace detail

template <typename T>
struct traits<T, typename std::enable_if<std::is_arithmetic<T>::value || std::is_enum<T>::value>::type>
    : detail::common_traits<T, true> {};

// A.1.1.2: strings.
template <typename CharT>
struct traits<std::basic_string<CharT>> : detail::common_traits<std::basic_string<CharT>, false> {
    using element_traits = traits<CharT>;
    using is_bounded = std::false_type;
};
template <typename CharT, std::size_t N>
struct traits<omg::types::basic_bounded_string<CharT, N>>
    : detail::common_traits<omg::types::basic_bounded_string<CharT, N>, false> {
    using element_traits = traits<CharT>;
    using is_bounded = std::true_type;
    using bound = std::integral_constant<std::uint32_t, static_cast<std::uint32_t>(N)>;
};

// A.1.1.3: sequences.
template <typename T>
struct traits<std::vector<T>> : detail::common_traits<std::vector<T>, false> {
    using element_traits = traits<T>;
    using is_bounded = std::false_type;
};
template <typename T, std::size_t N>
struct traits<omg::types::bounded_sequence<T, N>>
    : detail::common_traits<omg::types::bounded_sequence<T, N>, false> {
    using element_traits = traits<T>;
    using is_bounded = std::true_type;
    using bound = std::integral_constant<std::uint32_t, static_cast<std::uint32_t>(N)>;
};

// A.1.1.4: arrays. "The element_traits for a multidimensional array shall
// be the type trait for the core type."
namespace detail {
template <typename T>
struct core_of {
    using type = T;
};
template <typename T, std::size_t N>
struct core_of<std::array<T, N>> : core_of<T> {};
}  // namespace detail
template <typename T, std::size_t N>
struct traits<std::array<T, N>> : detail::common_traits<std::array<T, N>, false> {
    using element_traits = traits<typename detail::core_of<T>::type>;
    using dimensions = std::integral_constant<std::uint32_t,
                                              static_cast<std::uint32_t>(omg::types::dimensions<std::array<T, N>>::value)>;
};

// A.1.1.7: maps.
template <typename K, typename V>
struct traits<std::map<K, V>> : detail::common_traits<std::map<K, V>, false> {
    using key_traits = traits<K>;
    using value_traits = traits<V>;
    using is_bounded = std::false_type;
};
template <typename K, typename V, std::size_t N>
struct traits<omg::types::bounded_map<K, V, N>>
    : detail::common_traits<omg::types::bounded_map<K, V, N>, false> {
    using key_traits = traits<K>;
    using value_traits = traits<V>;
    using is_bounded = std::true_type;
    using bound = std::integral_constant<std::uint32_t, static_cast<std::uint32_t>(N)>;
};

// fixed and any are structured: passed by `const T&`.
template <>
struct traits<omg::types::fixed> : detail::common_traits<omg::types::fixed, false> {};
template <std::uint16_t D, std::uint16_t S>
struct traits<omg::types::fixed_n<D, S>> : detail::common_traits<omg::types::fixed_n<D, S>, false> {};

// -- A.1.2: exceptions ------------------------------------------------------------

class Exception : public std::exception {
public:
    ~Exception() override = default;
    virtual void raise() const = 0;
    virtual const char* _name() const { return "Exception"; }
    virtual const char* _rep_id() const { return "IDL:omg.org/CORBA/Exception:1.0"; }
    const char* what() const noexcept override { return _name(); }
    // A.1.2.2: "Any insertion ... for Exception ... shall result in both the
    // TypeCode and the value for the actual derived exception type being
    // stored into the Any": each exception class inserts itself.
    virtual void _insert_into(Any& any) const = 0;

protected:
    Exception() = default;
    Exception(const Exception&) = default;
    Exception(Exception&&) = default;
    Exception& operator=(const Exception&) = default;
    Exception& operator=(Exception&&) = default;
};

class UserException : public Exception {
public:
    ~UserException() override = default;
    const char* _name() const override { return "UserException"; }
    const char* _rep_id() const override { return "IDL:omg.org/CORBA/UserException:1.0"; }

protected:
    UserException() = default;
    UserException(const UserException&) = default;
    UserException(UserException&&) = default;
    UserException& operator=(const UserException&) = default;
    UserException& operator=(UserException&&) = default;
};

enum class CompletionStatus : std::uint32_t { COMPLETED_YES, COMPLETED_NO, COMPLETED_MAYBE };

class SystemException : public Exception {
public:
    ~SystemException() override = default;
    std::uint32_t minor() const { return minor_; }
    void minor(std::uint32_t value) { minor_ = value; }
    void raise() const override = 0;
    const char* what() const noexcept override { return _name(); }
    CompletionStatus completed() const { return completed_; }
    void completed(CompletionStatus value) { completed_ = value; }

protected:
    // "The default constructor for SystemException causes minor() to return
    // 0 and completed() to return COMPLETED_NO."
    SystemException() : minor_(0), completed_(CompletionStatus::COMPLETED_NO) {}
    SystemException(const SystemException&) = default;
    SystemException(SystemException&&) = default;
    SystemException(std::uint32_t minor, CompletionStatus status) : minor_(minor), completed_(status) {}
    SystemException& operator=(const SystemException&) = default;
    SystemException& operator=(SystemException&&) = default;

private:
    std::uint32_t minor_;
    CompletionStatus completed_;
};

// "Each specific system exception is derived from SystemException": the
// standard exceptions of CORBA 3.3 part 1 §2.4.3 (CORBA_StandardExceptions.idl).
#define ZERODDS_CORBA_SYSTEM_EXCEPTION(NAME)                                                       \
    class NAME final : public SystemException {                                                    \
    public:                                                                                        \
        NAME() = default;                                                                          \
        NAME(std::uint32_t minor, CompletionStatus status) : SystemException(minor, status) {}     \
        void raise() const override { throw *this; }                                               \
        const char* _name() const override { return #NAME; }                                       \
        const char* _rep_id() const override { return "IDL:omg.org/CORBA/" #NAME ":1.0"; }         \
        void _insert_into(Any& any) const override;                                                \
    };

ZERODDS_CORBA_SYSTEM_EXCEPTION(UNKNOWN)
ZERODDS_CORBA_SYSTEM_EXCEPTION(BAD_PARAM)
ZERODDS_CORBA_SYSTEM_EXCEPTION(NO_MEMORY)
ZERODDS_CORBA_SYSTEM_EXCEPTION(IMP_LIMIT)
ZERODDS_CORBA_SYSTEM_EXCEPTION(COMM_FAILURE)
ZERODDS_CORBA_SYSTEM_EXCEPTION(INV_OBJREF)
ZERODDS_CORBA_SYSTEM_EXCEPTION(NO_PERMISSION)
ZERODDS_CORBA_SYSTEM_EXCEPTION(INTERNAL)
ZERODDS_CORBA_SYSTEM_EXCEPTION(MARSHAL)
ZERODDS_CORBA_SYSTEM_EXCEPTION(INITIALIZE)
ZERODDS_CORBA_SYSTEM_EXCEPTION(NO_IMPLEMENT)
ZERODDS_CORBA_SYSTEM_EXCEPTION(BAD_TYPECODE)
ZERODDS_CORBA_SYSTEM_EXCEPTION(BAD_OPERATION)
ZERODDS_CORBA_SYSTEM_EXCEPTION(NO_RESOURCES)
ZERODDS_CORBA_SYSTEM_EXCEPTION(NO_RESPONSE)
ZERODDS_CORBA_SYSTEM_EXCEPTION(PERSIST_STORE)
ZERODDS_CORBA_SYSTEM_EXCEPTION(BAD_INV_ORDER)
ZERODDS_CORBA_SYSTEM_EXCEPTION(TRANSIENT)
ZERODDS_CORBA_SYSTEM_EXCEPTION(FREE_MEM)
ZERODDS_CORBA_SYSTEM_EXCEPTION(INV_IDENT)
ZERODDS_CORBA_SYSTEM_EXCEPTION(INV_FLAG)
ZERODDS_CORBA_SYSTEM_EXCEPTION(INTF_REPOS)
ZERODDS_CORBA_SYSTEM_EXCEPTION(BAD_CONTEXT)
ZERODDS_CORBA_SYSTEM_EXCEPTION(OBJ_ADAPTER)
ZERODDS_CORBA_SYSTEM_EXCEPTION(DATA_CONVERSION)
ZERODDS_CORBA_SYSTEM_EXCEPTION(OBJECT_NOT_EXIST)
ZERODDS_CORBA_SYSTEM_EXCEPTION(TRANSACTION_REQUIRED)
ZERODDS_CORBA_SYSTEM_EXCEPTION(TRANSACTION_ROLLEDBACK)
ZERODDS_CORBA_SYSTEM_EXCEPTION(INVALID_TRANSACTION)
ZERODDS_CORBA_SYSTEM_EXCEPTION(INV_POLICY)
ZERODDS_CORBA_SYSTEM_EXCEPTION(CODESET_INCOMPATIBLE)
ZERODDS_CORBA_SYSTEM_EXCEPTION(REBIND)
ZERODDS_CORBA_SYSTEM_EXCEPTION(TIMEOUT)
ZERODDS_CORBA_SYSTEM_EXCEPTION(TRANSACTION_UNAVAILABLE)
ZERODDS_CORBA_SYSTEM_EXCEPTION(TRANSACTION_MODE)
ZERODDS_CORBA_SYSTEM_EXCEPTION(BAD_QOS)
ZERODDS_CORBA_SYSTEM_EXCEPTION(INVALID_ACTIVITY)
ZERODDS_CORBA_SYSTEM_EXCEPTION(ACTIVITY_COMPLETED)
ZERODDS_CORBA_SYSTEM_EXCEPTION(ACTIVITY_REQUIRED)
ZERODDS_CORBA_SYSTEM_EXCEPTION(THREAD_CANCELLED)

#undef ZERODDS_CORBA_SYSTEM_EXCEPTION

namespace detail {
inline void bound_exceeded(const char*, std::size_t, std::size_t) {
    throw BAD_PARAM(0, CompletionStatus::COMPLETED_NO);
}
inline void nil_reference() { throw INV_OBJREF(0, CompletionStatus::COMPLETED_NO); }
}  // namespace detail


// -- A.1.3: TypeCode ------------------------------------------------------------------

// CORBA 3.3 part 1 §8.11 (CORBA_TypeCode.idl), mapped by clause 7.
enum class TCKind : std::uint32_t {
    tk_null,
    tk_void,
    tk_short,
    tk_long,
    tk_ushort,
    tk_ulong,
    tk_float,
    tk_double,
    tk_boolean,
    tk_char,
    tk_octet,
    tk_any,
    tk_TypeCode,
    tk_Principal,
    tk_objref,
    tk_struct,
    tk_union,
    tk_enum,
    tk_string,
    tk_sequence,
    tk_array,
    tk_alias,
    tk_except,
    tk_longlong,
    tk_ulonglong,
    tk_longdouble,
    tk_wchar,
    tk_wstring,
    tk_fixed,
    tk_value,
    tk_value_box,
    tk_native,
    tk_abstract_interface,
    tk_local_interface,
    tk_component,
    tk_home,
    tk_event
};
// CORBA 3.3 part 1, 11.2: "Each language mapping specifies what
// OpaqueValue maps to". IDL4 to C++ 1.0 does not name it; it is what the
// C++ mapping of CORBA makes it.
using OpaqueValue = void*;

using Visibility = std::int16_t;
constexpr Visibility PRIVATE_MEMBER = 0;
constexpr Visibility PUBLIC_MEMBER = 1;
using ValueModifier = std::int16_t;
constexpr ValueModifier VM_NONE = 0;
constexpr ValueModifier VM_CUSTOM = 1;
constexpr ValueModifier VM_ABSTRACT = 2;
constexpr ValueModifier VM_TRUNCATABLE = 3;

// "A TypeCode is a pseudo object ... implemented as a reference type ...
// For the strong reference, the type omg::types::ref_type<Y> shall be used.
// No public constructors for TypeCodes are defined." The static create_
// functions are the creation operations CORBA 3.3 part 1 §8.11.3 gives the
// ORB, which ORB::create_*_tc forward to.
class TypeCode final : public std::enable_shared_from_this<TypeCode> {
public:
    using ref = omg::types::ref_type<TypeCode>;

    class Bounds final : public UserException {
    public:
        void raise() const override { throw *this; }
        const char* _name() const override { return "Bounds"; }
        const char* _rep_id() const override { return "IDL:omg.org/CORBA/TypeCode/Bounds:1.0"; }
        void _insert_into(Any& any) const override;
    };
    class BadKind final : public UserException {
    public:
        void raise() const override { throw *this; }
        const char* _name() const override { return "BadKind"; }
        const char* _rep_id() const override { return "IDL:omg.org/CORBA/TypeCode/BadKind:1.0"; }
        void _insert_into(Any& any) const override;
    };

    // One member of a structure, union, exception or value type, as the
    // create_ functions take it; `label` only for a union, `visibility` only
    // for a value type.
    struct Member {
        std::string name;
        ref type;
        std::shared_ptr<Any> label;
        Visibility visibility;
    };

    bool equal(ref tc) const { return compare(*this, tc.get(), false); }
    bool equivalent(ref tc) const { return compare(*this, tc.get(), true); }
    ref get_compact_typecode() const;
    TCKind kind() const { return definition().kind_; }
    const std::string& id() const {
        const TypeCode& d = definition();
        d.require(has_id(d.kind_));
        return d.id_;
    }
    const std::string& name() const {
        const TypeCode& d = definition();
        d.require(has_id(d.kind_));
        return d.name_;
    }
    std::uint32_t member_count() const {
        const TypeCode& d = definition();
        d.require(has_members(d.kind_));
        return static_cast<std::uint32_t>(d.members_.size());
    }
    const std::string& member_name(std::uint32_t index) const {
        const TypeCode& d = definition();
        d.require(has_members(d.kind_));
        return d.member(index).name;
    }
    ref member_type(std::uint32_t index) const {
        const TypeCode& d = definition();
        d.require(has_members(d.kind_) && d.kind_ != TCKind::tk_enum);
        return d.member(index).type;
    }
    const Any& member_label(std::uint32_t index) const;
    ref discriminator_type() const {
        const TypeCode& d = definition();
        d.require(d.kind_ == TCKind::tk_union);
        return d.content_;
    }
    std::int32_t default_index() const {
        const TypeCode& d = definition();
        d.require(d.kind_ == TCKind::tk_union);
        return d.default_index_;
    }
    std::uint32_t length() const {
        const TypeCode& d = definition();
        d.require(d.kind_ == TCKind::tk_string || d.kind_ == TCKind::tk_wstring ||
                  d.kind_ == TCKind::tk_sequence || d.kind_ == TCKind::tk_array);
        return d.length_;
    }
    ref content_type() const {
        const TypeCode& d = definition();
        d.require(d.kind_ == TCKind::tk_sequence || d.kind_ == TCKind::tk_array ||
                  d.kind_ == TCKind::tk_value_box || d.kind_ == TCKind::tk_alias);
        return d.content_;
    }
    std::uint16_t fixed_digits() const {
        const TypeCode& d = definition();
        d.require(d.kind_ == TCKind::tk_fixed);
        return d.digits_;
    }
    std::int16_t fixed_scale() const {
        const TypeCode& d = definition();
        d.require(d.kind_ == TCKind::tk_fixed);
        return d.scale_;
    }
    Visibility member_visibility(std::uint32_t index) const {
        const TypeCode& d = definition();
        d.require(d.kind_ == TCKind::tk_value || d.kind_ == TCKind::tk_event);
        return d.member(index).visibility;
    }
    ValueModifier type_modifier() const {
        const TypeCode& d = definition();
        d.require(d.kind_ == TCKind::tk_value || d.kind_ == TCKind::tk_event);
        return d.modifier_;
    }
    ref concrete_base_type() const {
        const TypeCode& d = definition();
        d.require(d.kind_ == TCKind::tk_value || d.kind_ == TCKind::tk_event);
        return d.base_;
    }

    // -- creation, CORBA 3.3 part 1 §8.11.3 --

    // A kind without parameters; tk_string and tk_wstring unbounded,
    // tk_objref the one of Object.
    static ref get_primitive_tc(TCKind kind) {
        switch (kind) {
            case TCKind::tk_null:
            case TCKind::tk_void:
            case TCKind::tk_short:
            case TCKind::tk_long:
            case TCKind::tk_ushort:
            case TCKind::tk_ulong:
            case TCKind::tk_float:
            case TCKind::tk_double:
            case TCKind::tk_boolean:
            case TCKind::tk_char:
            case TCKind::tk_octet:
            case TCKind::tk_any:
            case TCKind::tk_TypeCode:
            case TCKind::tk_Principal:
            case TCKind::tk_longlong:
            case TCKind::tk_ulonglong:
            case TCKind::tk_longdouble:
            case TCKind::tk_wchar:
                return make(kind);
            case TCKind::tk_string:
                return create_string_tc(0);
            case TCKind::tk_wstring:
                return create_wstring_tc(0);
            case TCKind::tk_objref:
                return create_interface_tc("IDL:omg.org/CORBA/Object:1.0", "Object");
            default:
                throw BAD_PARAM(0, CompletionStatus::COMPLETED_NO);
        }
    }
    static ref create_struct_tc(const std::string& id, const std::string& name, std::vector<Member> members) {
        return aggregate(TCKind::tk_struct, id, name, std::move(members));
    }
    static ref create_exception_tc(const std::string& id, const std::string& name, std::vector<Member> members) {
        return aggregate(TCKind::tk_except, id, name, std::move(members));
    }
    // "For the default member, the label is the zero octet" (§8.11.1): a
    // member whose label is an octet 0 is the default one.
    static ref create_union_tc(const std::string& id, const std::string& name, ref discriminator,
                               std::vector<Member> members);
    static ref create_enum_tc(const std::string& id, const std::string& name,
                              const std::vector<std::string>& enumerators) {
        ref tc = named(TCKind::tk_enum, id, name);
        for (const std::string& enumerator : enumerators) {
            tc->members_.push_back(Member{enumerator, nullptr, nullptr, PUBLIC_MEMBER});
        }
        return tc;
    }
    static ref create_alias_tc(const std::string& id, const std::string& name, ref original) {
        ref tc = named(TCKind::tk_alias, id, name);
        tc->content_ = std::move(original);
        return bind_recursive(tc);
    }
    static ref create_interface_tc(const std::string& id, const std::string& name) {
        return named(TCKind::tk_objref, id, name);
    }
    static ref create_string_tc(std::uint32_t bound) {
        ref tc = make(TCKind::tk_string);
        tc->length_ = bound;
        return tc;
    }
    static ref create_wstring_tc(std::uint32_t bound) {
        ref tc = make(TCKind::tk_wstring);
        tc->length_ = bound;
        return tc;
    }
    static ref create_fixed_tc(std::uint16_t digits, std::int16_t scale) {
        ref tc = make(TCKind::tk_fixed);
        tc->digits_ = digits;
        tc->scale_ = scale;
        return tc;
    }
    static ref create_sequence_tc(std::uint32_t bound, ref element) {
        ref tc = make(TCKind::tk_sequence);
        tc->length_ = bound;
        tc->content_ = std::move(element);
        return tc;
    }
    static ref create_array_tc(std::uint32_t length, ref element) {
        ref tc = make(TCKind::tk_array);
        tc->length_ = length;
        tc->content_ = std::move(element);
        return tc;
    }
    static ref create_value_tc(const std::string& id, const std::string& name, ValueModifier modifier,
                               ref concrete_base, std::vector<Member> members) {
        return value(TCKind::tk_value, id, name, modifier, std::move(concrete_base), std::move(members));
    }
    static ref create_event_tc(const std::string& id, const std::string& name, ValueModifier modifier,
                               ref concrete_base, std::vector<Member> members) {
        return value(TCKind::tk_event, id, name, modifier, std::move(concrete_base), std::move(members));
    }
    static ref create_value_box_tc(const std::string& id, const std::string& name, ref boxed) {
        ref tc = named(TCKind::tk_value_box, id, name);
        tc->content_ = std::move(boxed);
        return bind_recursive(tc);
    }
    static ref create_native_tc(const std::string& id, const std::string& name) {
        return named(TCKind::tk_native, id, name);
    }
    static ref create_abstract_interface_tc(const std::string& id, const std::string& name) {
        return named(TCKind::tk_abstract_interface, id, name);
    }
    static ref create_local_interface_tc(const std::string& id, const std::string& name) {
        return named(TCKind::tk_local_interface, id, name);
    }
    static ref create_component_tc(const std::string& id, const std::string& name) {
        return named(TCKind::tk_component, id, name);
    }
    static ref create_home_tc(const std::string& id, const std::string& name) {
        return named(TCKind::tk_home, id, name);
    }
    // A placeholder for the type code with repository id `id` that contains
    // it; creating that type code binds it. Until then every operation
    // raises BAD_TYPECODE.
    static ref create_recursive_tc(const std::string& id) {
        ref tc = make(TCKind::tk_null);
        tc->recursive_id_ = id;
        tc->recursive_ = true;
        return tc;
    }

private:
    explicit TypeCode(TCKind kind)
        : kind_(kind), default_index_(-1), length_(0), digits_(0), scale_(0), modifier_(VM_NONE),
          recursive_(false) {}

    static ref make(TCKind kind) { return std::shared_ptr<TypeCode>(new TypeCode(kind)); }
    static ref named(TCKind kind, const std::string& id, const std::string& name) {
        ref tc = make(kind);
        tc->id_ = id;
        tc->name_ = name;
        return tc;
    }
    static ref aggregate(TCKind kind, const std::string& id, const std::string& name, std::vector<Member> members) {
        ref tc = named(kind, id, name);
        tc->members_ = std::move(members);
        return bind_recursive(tc);
    }
    static ref value(TCKind kind, const std::string& id, const std::string& name, ValueModifier modifier,
                     ref base, std::vector<Member> members) {
        ref tc = named(kind, id, name);
        tc->modifier_ = modifier;
        tc->base_ = std::move(base);
        tc->members_ = std::move(members);
        return bind_recursive(tc);
    }
    static bool has_id(TCKind k) {
        return k == TCKind::tk_objref || k == TCKind::tk_struct || k == TCKind::tk_union ||
               k == TCKind::tk_enum || k == TCKind::tk_alias || k == TCKind::tk_value ||
               k == TCKind::tk_value_box || k == TCKind::tk_native || k == TCKind::tk_abstract_interface ||
               k == TCKind::tk_local_interface || k == TCKind::tk_except || k == TCKind::tk_component ||
               k == TCKind::tk_home || k == TCKind::tk_event;
    }
    static bool has_members(TCKind k) {
        return k == TCKind::tk_struct || k == TCKind::tk_union || k == TCKind::tk_enum ||
               k == TCKind::tk_value || k == TCKind::tk_event || k == TCKind::tk_except;
    }
    // The type code a recursive placeholder stands for; BAD_TYPECODE while
    // it is unbound.
    const TypeCode& definition() const {
        const TypeCode* tc = this;
        for (int depth = 0; tc->recursive_; ++depth) {
            omg::types::ref_type<TypeCode> target = tc->target_.lock();
            if (!target || depth > 64) throw BAD_TYPECODE(0, CompletionStatus::COMPLETED_NO);
            tc = target.get();
        }
        return *tc;
    }
    void require(bool ok) const {
        if (!ok) throw BadKind();
    }
    const Member& member(std::uint32_t index) const {
        if (index >= members_.size()) throw Bounds();
        return members_[index];
    }
    static ref bind_recursive(const ref& tc) {
        if (!tc->id_.empty()) {
            std::set<const TypeCode*> seen;
            bind(tc.get(), tc->id_, tc, seen);
        }
        return tc;
    }
    static void bind(TypeCode* at, const std::string& id, const ref& to, std::set<const TypeCode*>& seen) {
        if (at == nullptr || !seen.insert(at).second) return;
        if (at->recursive_) {
            if (at->target_.expired() && at->recursive_id_ == id) at->target_ = to;
            return;
        }
        for (Member& m : at->members_) bind(m.type.get(), id, to, seen);
        bind(at->content_.get(), id, to, seen);
        bind(at->base_.get(), id, to, seen);
    }
    static bool compare(const TypeCode& a, const TypeCode* b, bool equivalent);
    static bool compare_at(const TypeCode& a, const TypeCode& b, bool equivalent,
                           std::set<std::pair<const TypeCode*, const TypeCode*>>& assumed);
    static ref compact(const TypeCode& tc, std::vector<std::pair<const TypeCode*, ref>>& done);

    TCKind kind_;
    std::string id_;
    std::string name_;
    std::vector<Member> members_;
    std::int32_t default_index_;
    std::uint32_t length_;
    // The content type of a sequence, array, value box and alias; the
    // discriminator type of a union.
    ref content_;
    std::uint16_t digits_;
    std::int16_t scale_;
    ValueModifier modifier_;
    ref base_;
    bool recursive_;
    std::string recursive_id_;
    std::weak_ptr<TypeCode> target_;
};

// The _tc_ constants of the basic types (A.1.3: "for each basic and defined
// IDL type, an implementation provides access to a TypeCode reference ...
// of the form _tc_<type>").
static const omg::types::ref_type<TypeCode> _tc_null = TypeCode::get_primitive_tc(TCKind::tk_null);
static const omg::types::ref_type<TypeCode> _tc_void = TypeCode::get_primitive_tc(TCKind::tk_void);
static const omg::types::ref_type<TypeCode> _tc_short = TypeCode::get_primitive_tc(TCKind::tk_short);
static const omg::types::ref_type<TypeCode> _tc_long = TypeCode::get_primitive_tc(TCKind::tk_long);
static const omg::types::ref_type<TypeCode> _tc_longlong = TypeCode::get_primitive_tc(TCKind::tk_longlong);
static const omg::types::ref_type<TypeCode> _tc_ushort = TypeCode::get_primitive_tc(TCKind::tk_ushort);
static const omg::types::ref_type<TypeCode> _tc_ulong = TypeCode::get_primitive_tc(TCKind::tk_ulong);
static const omg::types::ref_type<TypeCode> _tc_ulonglong = TypeCode::get_primitive_tc(TCKind::tk_ulonglong);
static const omg::types::ref_type<TypeCode> _tc_float = TypeCode::get_primitive_tc(TCKind::tk_float);
static const omg::types::ref_type<TypeCode> _tc_double = TypeCode::get_primitive_tc(TCKind::tk_double);
static const omg::types::ref_type<TypeCode> _tc_longdouble = TypeCode::get_primitive_tc(TCKind::tk_longdouble);
static const omg::types::ref_type<TypeCode> _tc_boolean = TypeCode::get_primitive_tc(TCKind::tk_boolean);
static const omg::types::ref_type<TypeCode> _tc_char = TypeCode::get_primitive_tc(TCKind::tk_char);
static const omg::types::ref_type<TypeCode> _tc_wchar = TypeCode::get_primitive_tc(TCKind::tk_wchar);
static const omg::types::ref_type<TypeCode> _tc_octet = TypeCode::get_primitive_tc(TCKind::tk_octet);
static const omg::types::ref_type<TypeCode> _tc_any = TypeCode::get_primitive_tc(TCKind::tk_any);
static const omg::types::ref_type<TypeCode> _tc_TypeCode = TypeCode::get_primitive_tc(TCKind::tk_TypeCode);
static const omg::types::ref_type<TypeCode> _tc_string = TypeCode::get_primitive_tc(TCKind::tk_string);
static const omg::types::ref_type<TypeCode> _tc_wstring = TypeCode::get_primitive_tc(TCKind::tk_wstring);
static const omg::types::ref_type<TypeCode> _tc_Object = TypeCode::get_primitive_tc(TCKind::tk_objref);
static const omg::types::ref_type<TypeCode> _tc_ValueBase =
    TypeCode::create_value_tc("IDL:omg.org/CORBA/ValueBase:1.0", "ValueBase", VM_NONE, nullptr, {});
static const omg::types::ref_type<TypeCode> _tc_AbstractBase =
    TypeCode::create_abstract_interface_tc("IDL:omg.org/CORBA/AbstractBase:1.0", "AbstractBase");


// -- A.1.7: Any -----------------------------------------------------------------------

// "The IDL type any maps to a C++ class named CORBA::Any": a value, its
// C++ type and its TypeCode. The insertion and extraction operators below
// cover the basic types, strings, Any and TypeCode; the generated code
// writes them for each type it declares, over _insert and _extract.
class Any {
public:
    Any() : type_(_tc_null) {}
    Any(const Any& other) : type_(other.type_), held_(other.held_ ? other.held_->clone() : nullptr) {}
    Any(Any&& other) noexcept : type_(std::move(other.type_)), held_(std::move(other.held_)) {
        other.type_ = _tc_null;
    }
    Any& operator=(const Any& other) {
        if (this != &other) {
            Any copy(other);
            swap(copy);
        }
        return *this;
    }
    Any& operator=(Any&& other) noexcept {
        if (this != &other) {
            type_ = std::move(other.type_);
            held_ = std::move(other.held_);
            other.type_ = _tc_null;
        }
        return *this;
    }
    ~Any() = default;

    // A.1.7.4: "The type accessor function returns a TypeCode reference to
    // the TypeCode associated with the Any."
    omg::types::ref_type<TypeCode> type() const { return type_; }
    // "If TypeCode::equivalent returns true, the type modifier function
    // replaces the original TypeCode ... If the two TypeCode are not
    // equivalent, the type modifier function raises the BAD_TYPECODE
    // exception."
    void type(omg::types::ref_type<TypeCode> tc) {
        if (!tc || !type_->equivalent(tc)) throw BAD_TYPECODE(0, CompletionStatus::COMPLETED_NO);
        type_ = std::move(tc);
    }

    // The value `value` with the TypeCode `tc`: what the insertion
    // operators do.
    template <typename T>
    void _insert(T value, omg::types::ref_type<TypeCode> tc) {
        held_.reset(new holder<T>(std::move(value)));
        type_ = std::move(tc);
    }
    // The value when the any holds a T; false, `out` unchanged, otherwise.
    template <typename T>
    bool _extract(T& out) const {
        const holder<T>* h = held_ ? dynamic_cast<const holder<T>*>(held_.get()) : nullptr;
        if (h == nullptr) return false;
        out = h->value;
        return true;
    }
    // A pointer to the value when the any holds a T, nullptr otherwise.
    template <typename T>
    const T* _value() const {
        const holder<T>* h = held_ ? dynamic_cast<const holder<T>*>(held_.get()) : nullptr;
        return h == nullptr ? nullptr : &h->value;
    }
    void swap(Any& other) noexcept {
        std::swap(type_, other.type_);
        std::swap(held_, other.held_);
    }
    // The held value as a system exception, when it is one.
    const SystemException* _system_exception() const {
        return held_ ? held_->system_exception() : nullptr;
    }
    // Two anys are equal when both are empty, or both hold a value of one
    // C++ type with equal TypeCodes and the values compare equal: what a
    // structure's comparison (7.2.4.3.1) asks of an `any` member. A held
    // type without operator== compares unequal.
    bool operator==(const Any& other) const {
        if (!held_ || !other.held_) return !held_ && !other.held_;
        return type_->equal(other.type_) && held_->equals(*other.held_);
    }
    bool operator!=(const Any& other) const { return !(*this == other); }

private:
    struct holder_base {
        virtual ~holder_base() = default;
        virtual std::unique_ptr<holder_base> clone() const = 0;
        virtual const SystemException* system_exception() const = 0;
        virtual bool equals(const holder_base& other) const = 0;
    };
    template <typename T>
    static auto same(const T& a, const T& b, int) -> decltype(static_cast<bool>(a == b)) {
        return static_cast<bool>(a == b);
    }
    template <typename T>
    static bool same(const T&, const T&, long) {
        return false;
    }
    template <typename T>
    static const SystemException* as_system(const T& value, std::true_type) {
        return &value;
    }
    template <typename T>
    static const SystemException* as_system(const T&, std::false_type) {
        return nullptr;
    }
    template <typename T>
    struct holder final : holder_base {
        explicit holder(T v) : value(std::move(v)) {}
        std::unique_ptr<holder_base> clone() const override {
            return std::unique_ptr<holder_base>(new holder(value));
        }
        const SystemException* system_exception() const override {
            return as_system(value, std::is_base_of<SystemException, T>());
        }
        bool equals(const holder_base& other) const override {
            const holder* o = dynamic_cast<const holder*>(&other);
            return o != nullptr && same(value, o->value, 0);
        }
        T value;
    };
    omg::types::ref_type<TypeCode> type_;
    std::unique_ptr<holder_base> held_;
};

inline void swap(Any& a, Any& b) noexcept { a.swap(b); }

// A.1.7.2 and A.1.7.3 for the types "normally passed by value" and for
// strings, wstrings and any.
#define ZERODDS_CORBA_ANY_BASIC(TYPE, TC)                                                       \
    inline void operator<<=(Any& any, TYPE value) { any._insert<TYPE>(value, TC); }              \
    inline bool operator>>=(const Any& any, TYPE& value) { return any._extract<TYPE>(value); }
ZERODDS_CORBA_ANY_BASIC(std::int16_t, _tc_short)
ZERODDS_CORBA_ANY_BASIC(std::int32_t, _tc_long)
ZERODDS_CORBA_ANY_BASIC(std::int64_t, _tc_longlong)
ZERODDS_CORBA_ANY_BASIC(std::uint16_t, _tc_ushort)
ZERODDS_CORBA_ANY_BASIC(std::uint32_t, _tc_ulong)
ZERODDS_CORBA_ANY_BASIC(std::uint64_t, _tc_ulonglong)
ZERODDS_CORBA_ANY_BASIC(float, _tc_float)
ZERODDS_CORBA_ANY_BASIC(double, _tc_double)
ZERODDS_CORBA_ANY_BASIC(long double, _tc_longdouble)
ZERODDS_CORBA_ANY_BASIC(char, _tc_char)
ZERODDS_CORBA_ANY_BASIC(wchar_t, _tc_wchar)
ZERODDS_CORBA_ANY_BASIC(bool, _tc_boolean)
ZERODDS_CORBA_ANY_BASIC(std::uint8_t, _tc_octet)
#undef ZERODDS_CORBA_ANY_BASIC

// int8 has no kind of its own in CORBA 3.3's TCKind; it travels as a short.
inline void operator<<=(Any& any, std::int8_t value) { any._insert<std::int8_t>(value, _tc_short); }
inline bool operator>>=(const Any& any, std::int8_t& value) { return any._extract<std::int8_t>(value); }

inline void operator<<=(Any& any, const std::string& value) { any._insert<std::string>(value, _tc_string); }
inline void operator<<=(Any& any, std::string&& value) { any._insert<std::string>(std::move(value), _tc_string); }
inline void operator<<=(Any& any, const char* value) { any._insert<std::string>(std::string(value), _tc_string); }
inline bool operator>>=(const Any& any, std::string& value) { return any._extract<std::string>(value); }
inline void operator<<=(Any& any, const std::wstring& value) { any._insert<std::wstring>(value, _tc_wstring); }
inline void operator<<=(Any& any, std::wstring&& value) {
    any._insert<std::wstring>(std::move(value), _tc_wstring);
}
inline bool operator>>=(const Any& any, std::wstring& value) { return any._extract<std::wstring>(value); }
inline void operator<<=(Any& any, const Any& value) { any._insert<Any>(value, _tc_any); }
inline void operator<<=(Any& any, Any&& value) { any._insert<Any>(std::move(value), _tc_any); }
inline bool operator>>=(const Any& any, Any& value) { return any._extract<Any>(value); }
inline void operator<<=(Any& any, omg::types::ref_type<TypeCode> value) {
    any._insert<omg::types::ref_type<TypeCode>>(std::move(value), _tc_TypeCode);
}
inline bool operator>>=(const Any& any, omg::types::ref_type<TypeCode>& value) {
    return any._extract<omg::types::ref_type<TypeCode>>(value);
}

// A.1.7.2 and A.1.7.3 "for each separate IDL type T": a sequence, an array,
// a bounded string and a fixed type are instances of C++ templates, whose
// operators are these templates. The TypeCode "is implied by the C++ type":
// the bound and the length are the template's arguments, the element's is
// the one its own insertion sets.
template <typename T, typename A>
inline void operator<<=(Any& any, const std::vector<T, A>& value);
template <typename T, typename A>
inline void operator<<=(Any& any, std::vector<T, A>&& value);
template <typename T, typename A>
inline bool operator>>=(const Any& any, std::vector<T, A>& value);
template <typename T, std::size_t N>
inline void operator<<=(Any& any, const omg::types::bounded_sequence<T, N>& value);
template <typename T, std::size_t N>
inline void operator<<=(Any& any, omg::types::bounded_sequence<T, N>&& value);
template <typename T, std::size_t N>
inline bool operator>>=(const Any& any, omg::types::bounded_sequence<T, N>& value);
template <typename T, std::size_t N>
inline void operator<<=(Any& any, const std::array<T, N>& value);
template <typename T, std::size_t N>
inline void operator<<=(Any& any, std::array<T, N>&& value);
template <typename T, std::size_t N>
inline bool operator>>=(const Any& any, std::array<T, N>& value);
template <typename CharT, std::size_t N>
inline void operator<<=(Any& any, const omg::types::basic_bounded_string<CharT, N>& value);
template <typename CharT, std::size_t N>
inline void operator<<=(Any& any, omg::types::basic_bounded_string<CharT, N>&& value);
template <typename CharT, std::size_t N>
inline bool operator>>=(const Any& any, omg::types::basic_bounded_string<CharT, N>& value);
template <std::uint16_t D, std::uint16_t S>
inline void operator<<=(Any& any, const omg::types::fixed_n<D, S>& value);
template <std::uint16_t D, std::uint16_t S>
inline bool operator>>=(const Any& any, omg::types::fixed_n<D, S>& value);

namespace detail {
// The TypeCode the insertion of a T sets.
template <typename T>
inline omg::types::ref_type<TypeCode> inserted_typecode() {
    static const omg::types::ref_type<TypeCode> tc = [] {
        Any any;
        any <<= T();
        return any.type();
    }();
    return tc;
}
inline omg::types::ref_type<TypeCode> string_typecode(char, std::uint32_t bound) {
    return TypeCode::create_string_tc(bound);
}
inline omg::types::ref_type<TypeCode> string_typecode(wchar_t, std::uint32_t bound) {
    return TypeCode::create_wstring_tc(bound);
}
}  // namespace detail

template <typename T, typename A>
inline void operator<<=(Any& any, const std::vector<T, A>& value) {
    any._insert<std::vector<T, A>>(value, TypeCode::create_sequence_tc(0, detail::inserted_typecode<T>()));
}
template <typename T, typename A>
inline void operator<<=(Any& any, std::vector<T, A>&& value) {
    any._insert<std::vector<T, A>>(std::move(value),
                                   TypeCode::create_sequence_tc(0, detail::inserted_typecode<T>()));
}
template <typename T, typename A>
inline bool operator>>=(const Any& any, std::vector<T, A>& value) {
    return any._extract<std::vector<T, A>>(value);
}
template <typename T, std::size_t N>
inline void operator<<=(Any& any, const omg::types::bounded_sequence<T, N>& value) {
    any._insert<omg::types::bounded_sequence<T, N>>(
        value, TypeCode::create_sequence_tc(static_cast<std::uint32_t>(N), detail::inserted_typecode<T>()));
}
template <typename T, std::size_t N>
inline void operator<<=(Any& any, omg::types::bounded_sequence<T, N>&& value) {
    any._insert<omg::types::bounded_sequence<T, N>>(
        std::move(value),
        TypeCode::create_sequence_tc(static_cast<std::uint32_t>(N), detail::inserted_typecode<T>()));
}
template <typename T, std::size_t N>
inline bool operator>>=(const Any& any, omg::types::bounded_sequence<T, N>& value) {
    return any._extract<omg::types::bounded_sequence<T, N>>(value);
}
template <typename T, std::size_t N>
inline void operator<<=(Any& any, const std::array<T, N>& value) {
    any._insert<std::array<T, N>>(
        value, TypeCode::create_array_tc(static_cast<std::uint32_t>(N), detail::inserted_typecode<T>()));
}
template <typename T, std::size_t N>
inline void operator<<=(Any& any, std::array<T, N>&& value) {
    any._insert<std::array<T, N>>(
        std::move(value), TypeCode::create_array_tc(static_cast<std::uint32_t>(N), detail::inserted_typecode<T>()));
}
template <typename T, std::size_t N>
inline bool operator>>=(const Any& any, std::array<T, N>& value) {
    return any._extract<std::array<T, N>>(value);
}
template <typename CharT, std::size_t N>
inline void operator<<=(Any& any, const omg::types::basic_bounded_string<CharT, N>& value) {
    any._insert<omg::types::basic_bounded_string<CharT, N>>(
        value, detail::string_typecode(CharT(), static_cast<std::uint32_t>(N)));
}
template <typename CharT, std::size_t N>
inline void operator<<=(Any& any, omg::types::basic_bounded_string<CharT, N>&& value) {
    any._insert<omg::types::basic_bounded_string<CharT, N>>(
        std::move(value), detail::string_typecode(CharT(), static_cast<std::uint32_t>(N)));
}
template <typename CharT, std::size_t N>
inline bool operator>>=(const Any& any, omg::types::basic_bounded_string<CharT, N>& value) {
    return any._extract<omg::types::basic_bounded_string<CharT, N>>(value);
}
template <std::uint16_t D, std::uint16_t S>
inline void operator<<=(Any& any, const omg::types::fixed_n<D, S>& value) {
    any._insert<omg::types::fixed_n<D, S>>(value, TypeCode::create_fixed_tc(D, static_cast<std::int16_t>(S)));
}
template <std::uint16_t D, std::uint16_t S>
inline bool operator>>=(const Any& any, omg::types::fixed_n<D, S>& value) {
    return any._extract<omg::types::fixed_n<D, S>>(value);
}

namespace detail {
// An any holding `value`: the label of a union member in its type code.
template <typename T>
inline Any any_of(T value) {
    Any any;
    any <<= value;
    return any;
}
}  // namespace detail

// -- TypeCode, the parts that need Any ----------------------------------------------------

inline const Any& TypeCode::member_label(std::uint32_t index) const {
    const TypeCode& d = definition();
    d.require(d.kind_ == TCKind::tk_union);
    const Member& m = d.member(index);
    if (!m.label) throw BAD_TYPECODE(0, CompletionStatus::COMPLETED_NO);
    return *m.label;
}

inline TypeCode::ref TypeCode::create_union_tc(const std::string& id, const std::string& name, ref discriminator,
                                               std::vector<Member> members) {
    ref tc = named(TCKind::tk_union, id, name);
    tc->content_ = std::move(discriminator);
    for (std::size_t i = 0; i < members.size(); ++i) {
        std::uint8_t zero = 1;
        const Any* label = members[i].label.get();
        if (label != nullptr && label->type()->kind() == TCKind::tk_octet && (*label >>= zero) && zero == 0) {
            tc->default_index_ = static_cast<std::int32_t>(i);
        }
    }
    tc->members_ = std::move(members);
    return bind_recursive(tc);
}

// equal: the same kind and every parameter the same, names and ids
// included. equivalent: aliases looked through, names left out, ids
// compared where both have one (CORBA 3.3 part 1 §8.11.2).
inline bool TypeCode::compare(const TypeCode& a, const TypeCode* b, bool equivalent) {
    if (b == nullptr) return false;
    std::set<std::pair<const TypeCode*, const TypeCode*>> assumed;
    return compare_at(a.definition(), b->definition(), equivalent, assumed);
}

inline bool TypeCode::compare_at(const TypeCode& a0, const TypeCode& b0, bool equivalent,
                                 std::set<std::pair<const TypeCode*, const TypeCode*>>& assumed) {
    const TypeCode* a = &a0.definition();
    const TypeCode* b = &b0.definition();
    if (equivalent) {
        for (int i = 0; a->kind_ == TCKind::tk_alias && a->content_ && i < 64; ++i) a = &a->content_->definition();
        for (int i = 0; b->kind_ == TCKind::tk_alias && b->content_ && i < 64; ++i) b = &b->content_->definition();
    }
    if (a == b) return true;
    if (!assumed.insert(std::make_pair(a, b)).second) return true;
    if (a->kind_ != b->kind_) return false;
    if (has_id(a->kind_)) {
        if (equivalent) {
            if (!a->id_.empty() && !b->id_.empty()) return a->id_ == b->id_;
        } else if (a->id_ != b->id_ || a->name_ != b->name_) {
            return false;
        }
    }
    if (a->length_ != b->length_ || a->digits_ != b->digits_ || a->scale_ != b->scale_ ||
        a->modifier_ != b->modifier_ || a->default_index_ != b->default_index_ ||
        a->members_.size() != b->members_.size()) {
        return false;
    }
    for (std::size_t i = 0; i < a->members_.size(); ++i) {
        const Member& x = a->members_[i];
        const Member& y = b->members_[i];
        if (!equivalent && x.name != y.name) return false;
        if (x.visibility != y.visibility) return false;
        if (static_cast<bool>(x.type) != static_cast<bool>(y.type)) return false;
        if (x.type && !compare_at(*x.type, *y.type, equivalent, assumed)) return false;
    }
    if (static_cast<bool>(a->content_) != static_cast<bool>(b->content_)) return false;
    if (a->content_ && !compare_at(*a->content_, *b->content_, equivalent, assumed)) return false;
    if (static_cast<bool>(a->base_) != static_cast<bool>(b->base_)) return false;
    if (a->base_ && !compare_at(*a->base_, *b->base_, equivalent, assumed)) return false;
    return true;
}

// "get_compact_typecode ... removes all optional name and member name
// fields": every name and member name empty, aliases kept.
inline TypeCode::ref TypeCode::get_compact_typecode() const {
    std::vector<std::pair<const TypeCode*, ref>> done;
    return compact(definition(), done);
}

inline TypeCode::ref TypeCode::compact(const TypeCode& tc, std::vector<std::pair<const TypeCode*, ref>>& done) {
    const TypeCode& d = tc.definition();
    for (const auto& entry : done) {
        if (entry.first == &d) return entry.second;
    }
    ref out = make(d.kind_);
    done.push_back(std::make_pair(&d, out));
    out->id_ = d.id_;
    out->default_index_ = d.default_index_;
    out->length_ = d.length_;
    out->digits_ = d.digits_;
    out->scale_ = d.scale_;
    out->modifier_ = d.modifier_;
    for (const Member& m : d.members_) {
        out->members_.push_back(Member{std::string(), m.type ? compact(*m.type, done) : nullptr, m.label, m.visibility});
    }
    if (d.content_) out->content_ = compact(*d.content_, done);
    if (d.base_) out->base_ = compact(*d.base_, done);
    return out;
}


// -- A.1.5, A.1.6, A.1.8.5, A.1.8.9.1, A.1.9.1: the bases ------------------------------

// "The CORBA Object interface ... shall be mapped to C++ according to the
// mapping rules for Interfaces – Full ... In addition to these rules, all
// operation names in interface Object have leading underscores." The
// generated class of an interface overrides _is_a and _repository_id with
// its repository ids; an object adapter overrides the rest for the objects
// it serves.
class Object : public std::enable_shared_from_this<Object> {
public:
    virtual ~Object() = default;

    virtual omg::types::ref_type<InterfaceDef> _get_interface() {
        throw NO_IMPLEMENT(0, CompletionStatus::COMPLETED_NO);
    }
    virtual bool _is_a(const std::string& logical_type_id) {
        return logical_type_id == "IDL:omg.org/CORBA/Object:1.0";
    }
    virtual bool _non_existent() { return false; }
    virtual bool _is_equivalent(omg::types::ref_type<Object> other_object) {
        return other_object.get() == this;
    }
    virtual std::uint32_t _hash(std::uint32_t maximum) {
        const std::uintptr_t address = reinterpret_cast<std::uintptr_t>(this);
        return maximum == 0 ? 0 : static_cast<std::uint32_t>(address % (static_cast<std::uintptr_t>(maximum) + 1));
    }
    virtual std::string _repository_id() { return "IDL:omg.org/CORBA/Object:1.0"; }
    // The ORB of this process (omg/corba_orb.hpp); INITIALIZE before
    // CORBA::ORB_init.
    virtual omg::types::ref_type<ORB> _get_orb();
    virtual omg::types::ref_type<Policy> _get_policy(std::uint32_t) {
        throw INV_POLICY(0, CompletionStatus::COMPLETED_NO);
    }
    virtual omg::types::ref_type<Object> _get_component() { return nullptr; }
    // A reference names its object or is nil (A.1.5.3); an operation
    // reached through a reference finds an object.
    virtual bool _is_nil() { return false; }
    virtual omg::types::ref_type<Object> _duplicate() { return shared_from_this(); }
    virtual void _release() {}
    // The dynamic invocation interface is the ORB's.
    virtual void _create_request(omg::types::ref_type<Context>, const std::string&, omg::types::ref_type<NVList>,
                                 NamedValue&, omg::types::ref_type<Request>&, std::uint32_t) {
        throw NO_IMPLEMENT(0, CompletionStatus::COMPLETED_NO);
    }
    virtual std::vector<omg::types::ref_type<DomainManager>> _get_domain_managers() {
        return std::vector<omg::types::ref_type<DomainManager>>();
    }
    virtual omg::types::ref_type<Object> _set_policy_overrides(const std::vector<omg::types::ref_type<Policy>>&,
                                                               SetOverrideType) {
        throw NO_IMPLEMENT(0, CompletionStatus::COMPLETED_NO);
    }
    virtual omg::types::ref_type<Policy> _get_client_policy(std::uint32_t) {
        throw INV_POLICY(0, CompletionStatus::COMPLETED_NO);
    }
    virtual std::vector<omg::types::ref_type<Policy>> _get_policy_overrides(const std::vector<std::uint32_t>&) {
        return std::vector<omg::types::ref_type<Policy>>();
    }
    // An object of this process has no connection to open.
    virtual bool _validate_connection(std::vector<omg::types::ref_type<Policy>>& inconsistent_policies) {
        inconsistent_policies.clear();
        return true;
    }

protected:
    Object() = default;
    Object(const Object&) = default;
    Object(Object&&) = default;
    Object& operator=(const Object&) = default;
    Object& operator=(Object&&) = default;
};

namespace detail {
// The ORB ORB_init made, which Object::_get_orb hands out.
inline omg::types::ref_type<ORB>& default_orb() {
    static omg::types::ref_type<ORB> orb;
    return orb;
}
}  // namespace detail

inline omg::types::ref_type<ORB> Object::_get_orb() {
    omg::types::ref_type<ORB> orb = detail::default_orb();
    if (!orb) throw INITIALIZE(0, CompletionStatus::COMPLETED_NO);
    return orb;
}

// A.1.6: "a class derived from Object that is used as a base class for
// locality constrained object implementations".
class LocalObject : public virtual Object {
public:
    ~LocalObject() override = default;
    std::string _repository_id() override { return "IDL:omg.org/CORBA/LocalObject:1.0"; }

protected:
    LocalObject() = default;
};

// A.1.8.5. A value knows the reference that holds it, which _to_value of
// an abstract interface it supports hands out (A.1.9.1).
class ValueBase : public std::enable_shared_from_this<ValueBase> {
public:
    // "The copy_value operation returns by default a nil valuetype reference."
    virtual omg::types::ref_type<ValueBase> _copy_value() { return nullptr; }

protected:
    ValueBase() = default;
    ValueBase(ValueBase&&) = default;
    ValueBase(const ValueBase&) = default;
    virtual ~ValueBase() = default;

private:
    ValueBase& operator=(ValueBase&&) = delete;
    ValueBase& operator=(const ValueBase&) = delete;
};

// A.1.9.1.
class AbstractBase {
public:
    // "If the concrete type of an abstract interface instance is a normal
    // object reference, the _to_object function returns a reference to that
    // object, otherwise it returns a nil reference."
    virtual omg::types::ref_type<Object> _to_object() { return nullptr; }
    // "If the concrete type is a valuetype, _to_value returns a reference to
    // that valuetype, otherwise it returns a nil reference."
    virtual omg::types::ref_type<ValueBase> _to_value() { return nullptr; }

protected:
    AbstractBase() = default;
    AbstractBase(const AbstractBase&) = default;
    AbstractBase(AbstractBase&&) = default;
    AbstractBase& operator=(const AbstractBase&) = default;
    AbstractBase& operator=(AbstractBase&&) = default;
    virtual ~AbstractBase() = default;
};

namespace detail {
struct factory_access;
}

// A.1.8.9.1: "The base of all value factory classes ... a protected
// constructor and destructor and deleted copy and move constructors and
// assignment operators"; create_for_unmarshal is private, for the
// implementation (detail::factory_access).
class ValueFactoryBase {
protected:
    virtual ~ValueFactoryBase() = default;
    ValueFactoryBase() = default;

private:
    friend struct detail::factory_access;
    virtual omg::types::ref_type<ValueBase> create_for_unmarshal() = 0;
    ValueFactoryBase(const ValueFactoryBase&) = delete;
    ValueFactoryBase(ValueFactoryBase&&) = delete;
    ValueFactoryBase& operator=(const ValueFactoryBase&) = delete;
    ValueFactoryBase& operator=(ValueFactoryBase&&) = delete;
};

namespace detail {
struct factory_access {
    static omg::types::ref_type<ValueBase> create_for_unmarshal(ValueFactoryBase& factory) {
        return factory.create_for_unmarshal();
    }
};

// Table A.13: a reference type, passed as its reference.
template <typename T>
struct reference_traits {
    using value_type = omg::types::ref_type<T>;
    using in_type = omg::types::ref_type<T>;
    using out_type = omg::types::ref_type<T>&;
    using inout_type = omg::types::ref_type<T>&;
    using ref_type = omg::types::ref_type<T>;
    using weak_ref_type = omg::types::weak_ref_type<T>;
};
}  // namespace detail

// A.1.1.1 and A.1.1.10 for the bases; the generated interfaces, value types
// and value boxes specialize traits the same way.
template <>
struct traits<Object> : detail::reference_traits<Object> {
    using is_local = std::false_type;
    using is_abstract = std::false_type;
    static omg::types::ref_type<Object> narrow(omg::types::ref_type<Object> p) { return p; }
};
template <>
struct traits<LocalObject> : detail::reference_traits<LocalObject> {
    using is_local = std::true_type;
    using is_abstract = std::false_type;
    static omg::types::ref_type<LocalObject> narrow(omg::types::ref_type<Object> p) {
        return std::dynamic_pointer_cast<LocalObject>(p);
    }
};
template <>
struct traits<AbstractBase> : detail::reference_traits<AbstractBase> {
    using is_local = std::false_type;
    using is_abstract = std::true_type;
};
template <>
struct traits<ValueBase> : detail::reference_traits<ValueBase> {
    static omg::types::ref_type<ValueBase> narrow(omg::types::ref_type<ValueBase> p) { return p; }
};
template <>
struct traits<TypeCode> : detail::reference_traits<TypeCode> {};

// A.1.7.2: object, value and abstract references are inserted with their
// type code; A.1.7.3: "Any object reference shall be extractable from an
// Any as a base object reference. Any abstract reference shall be
// extractable ... as a base object reference or as a base valuetype
// reference. Any valuetype reference shall be extractable ... as a base
// valuetype reference."
inline void operator<<=(Any& any, omg::types::ref_type<Object> value) {
    any._insert<omg::types::ref_type<Object>>(std::move(value), _tc_Object);
}
inline bool operator>>=(const Any& any, omg::types::ref_type<Object>& value) {
    if (any._extract<omg::types::ref_type<Object>>(value)) return true;
    const omg::types::ref_type<AbstractBase>* abstract = any._value<omg::types::ref_type<AbstractBase>>();
    if (abstract != nullptr) {
        value = *abstract ? (*abstract)->_to_object() : nullptr;
        return true;
    }
    return false;
}
inline void operator<<=(Any& any, omg::types::ref_type<ValueBase> value) {
    any._insert<omg::types::ref_type<ValueBase>>(std::move(value), _tc_ValueBase);
}
inline bool operator>>=(const Any& any, omg::types::ref_type<ValueBase>& value) {
    if (any._extract<omg::types::ref_type<ValueBase>>(value)) return true;
    const omg::types::ref_type<AbstractBase>* abstract = any._value<omg::types::ref_type<AbstractBase>>();
    if (abstract != nullptr) {
        value = *abstract ? (*abstract)->_to_value() : nullptr;
        return true;
    }
    return false;
}
inline void operator<<=(Any& any, omg::types::ref_type<AbstractBase> value) {
    any._insert<omg::types::ref_type<AbstractBase>>(std::move(value), _tc_AbstractBase);
}
inline bool operator>>=(const Any& any, omg::types::ref_type<AbstractBase>& value) {
    return any._extract<omg::types::ref_type<AbstractBase>>(value);
}

// A.1.2.1.
class UnknownUserException final : public UserException {
public:
    UnknownUserException() = default;
    explicit UnknownUserException(Any exception) : exception_(std::move(exception)) {}
    const Any& exception() const { return exception_; }
    void raise() const override { throw *this; }
    const char* _name() const override { return "UnknownUserException"; }
    const char* _rep_id() const override { return "IDL:omg.org/CORBA/UnknownUserException:1.0"; }
    // The exception it stands for goes in.
    void _insert_into(Any& any) const override { any = exception_; }

private:
    Any exception_;
};


// -- A.1.2.2: exceptions in an Any -------------------------------------------------------------

namespace detail {
// The type code of a standard exception: CORBA 3.3 part 1 §2.4.3,
// `exception NAME { unsigned long minor; completion_status completed; }`.
inline omg::types::ref_type<TypeCode> completion_status_tc() {
    static const omg::types::ref_type<TypeCode> tc = TypeCode::create_enum_tc(
        "IDL:omg.org/CORBA/completion_status:1.0", "completion_status",
        {"COMPLETED_YES", "COMPLETED_NO", "COMPLETED_MAYBE"});
    return tc;
}
inline omg::types::ref_type<TypeCode> system_exception_tc(const char* name) {
    return TypeCode::create_exception_tc(std::string("IDL:omg.org/CORBA/") + name + ":1.0", name,
                                         {TypeCode::Member{"minor", _tc_ulong, nullptr, PUBLIC_MEMBER},
                                          TypeCode::Member{"completed", completion_status_tc(), nullptr,
                                                           PUBLIC_MEMBER}});
}
}  // namespace detail

// "The following Any insertion for Exception shall be provided".
inline void operator<<=(Any& any, const Exception& exception) { exception._insert_into(any); }

#define ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(NAME)                                                   \
    inline void operator<<=(Any& any, const NAME& value) {                                       \
        any._insert<NAME>(value, detail::system_exception_tc(#NAME));                            \
    }                                                                                            \
    inline void operator<<=(Any& any, NAME&& value) {                                            \
        any._insert<NAME>(std::move(value), detail::system_exception_tc(#NAME));                 \
    }                                                                                            \
    inline bool operator>>=(const Any& any, NAME& value) { return any._extract<NAME>(value); }    \
    inline void NAME::_insert_into(Any& any) const { any <<= *this; }
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(UNKNOWN)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(BAD_PARAM)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(NO_MEMORY)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(IMP_LIMIT)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(COMM_FAILURE)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(INV_OBJREF)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(NO_PERMISSION)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(INTERNAL)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(MARSHAL)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(INITIALIZE)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(NO_IMPLEMENT)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(BAD_TYPECODE)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(BAD_OPERATION)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(NO_RESOURCES)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(NO_RESPONSE)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(PERSIST_STORE)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(BAD_INV_ORDER)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(TRANSIENT)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(FREE_MEM)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(INV_IDENT)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(INV_FLAG)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(INTF_REPOS)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(BAD_CONTEXT)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(OBJ_ADAPTER)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(DATA_CONVERSION)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(OBJECT_NOT_EXIST)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(TRANSACTION_REQUIRED)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(TRANSACTION_ROLLEDBACK)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(INVALID_TRANSACTION)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(INV_POLICY)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(CODESET_INCOMPATIBLE)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(REBIND)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(TIMEOUT)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(TRANSACTION_UNAVAILABLE)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(TRANSACTION_MODE)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(BAD_QOS)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(INVALID_ACTIVITY)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(ACTIVITY_COMPLETED)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(ACTIVITY_REQUIRED)
ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY(THREAD_CANCELLED)
#undef ZERODDS_CORBA_SYSTEM_EXCEPTION_ANY

// "bool operator>>=(const Any&, SystemException& se) ... returns true if the
// Any ... contains a system exception and the implementation has static type
// information for the actual system exception ... In that case, the operator
// assigns the actual exception to se": its minor code and completion status,
// which is what a SystemException holds.
inline bool operator>>=(const Any& any, SystemException& se) {
    const TCKind kind = any.type()->kind();
    if (kind != TCKind::tk_except) return false;
    const std::string id = any.type()->id();
    if (id.compare(0, 18, "IDL:omg.org/CORBA/") != 0) return false;
    const SystemException* held = any._system_exception();
    if (held == nullptr) {
        se.minor(0);
        se.completed(CompletionStatus::COMPLETED_MAYBE);
        return true;
    }
    se.minor(held->minor());
    se.completed(held->completed());
    return true;
}

inline void TypeCode::Bounds::_insert_into(Any& any) const {
    any._insert<Bounds>(*this, TypeCode::create_exception_tc(_rep_id(), _name(), {}));
}
inline void TypeCode::BadKind::_insert_into(Any& any) const {
    any._insert<BadKind>(*this, TypeCode::create_exception_tc(_rep_id(), _name(), {}));
}


}  // namespace CORBA

#endif  // OMG_CORBA_HPP
