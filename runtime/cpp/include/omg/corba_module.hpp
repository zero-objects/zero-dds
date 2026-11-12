// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
// Generated from IDL by ZeroDDS: IDL4 to C++ Language Mapping 1.0.
#ifndef ZERODDS_IDL4_CORBA_MODULE_HPP
#define ZERODDS_IDL4_CORBA_MODULE_HPP

#include <omg/corba.hpp>

namespace CORBA {
    class CustomMarshal;
    class DataInputStream;
    class DataOutputStream;
    class Pollable;
    class DIIPollable;
    struct StructMember;
    struct Initializer;
    struct UnionMember;
    struct ExceptionDescription;
    struct ExtInitializer;
    struct ModuleDescription;
    struct ConstantDescription;
    struct TypeDescription;
    struct AttributeDescription;
    struct ExtAttributeDescription;
    struct ParameterDescription;
    struct OperationDescription;
    struct InterfaceDescription;
    struct ValueMember;
    struct ValueDescription;
    struct NamedValue;
    struct ServiceDetail;
    struct ServiceInformation;
    class ConstructionPolicy;
    class Current;
    class DomainManager;
    class Policy;
    class PollableSet;
    class AbstractInterfaceDef;
    class AliasDef;
    class ArrayDef;
    class AttributeDef;
    class ConstantDef;
    class Contained;
    class Container;
    class EnumDef;
    class ExceptionDef;
    class ExtInterfaceDef;
    class ExtValueDef;
    class ExtAbstractInterfaceDef;
    class ExtLocalInterfaceDef;
    class FixedDef;
    class IDLType;
    class InterfaceDef;
    class IRObject;
    class LocalInterfaceDef;
    class ModuleDef;
    class NativeDef;
    class OperationDef;
    class PrimitiveDef;
    class Repository;
    class SequenceDef;
    class StringDef;
    class StructDef;
    class TypedefDef;
    class UnionDef;
    class ValueDef;
    class ValueBoxDef;
    class ValueMemberDef;
    class WstringDef;
    class PolicyManager;
    class PolicyCurrent;
    class ExtAttributeDef;
    class InterfaceAttrExtension;
    class NVList;
    class Context;
    class Request;
    class ORB;

    using PolicyType = std::uint32_t;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_PolicyType = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/PolicyType:1.0", "PolicyType", ::CORBA::_tc_ulong);

    class Policy : public virtual ::CORBA::Object {
        public:

        virtual ~Policy() = default;

        virtual ::CORBA::PolicyType policy_type() const = 0;

        virtual ::omg::types::ref_type<::CORBA::Policy> copy() = 0;

        virtual void destroy() = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/Policy:1.0" || ::CORBA::Object::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/Policy:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/Policy:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Policy = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/Policy:1.0", "Policy");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Policy> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_Policy); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Policy>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::Policy>(base); return !base || value; }

    class ConstructionPolicy : public virtual ::CORBA::Policy {
        public:

        virtual ~ConstructionPolicy() = default;

        virtual void make_domain_manager(::omg::types::ref_type<::CORBA::InterfaceDef> object_type, bool constr_policy) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ConstructionPolicy:1.0" || ::CORBA::Policy::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ConstructionPolicy:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ConstructionPolicy:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ConstructionPolicy = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ConstructionPolicy:1.0", "ConstructionPolicy");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ConstructionPolicy> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ConstructionPolicy); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ConstructionPolicy>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ConstructionPolicy>(base); return !base || value; }

    class Current : public virtual ::CORBA::LocalObject {
        public:

        virtual ~Current() = default;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/Current:1.0" || ::CORBA::LocalObject::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/Current:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/Current:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Current = ::CORBA::TypeCode::create_local_interface_tc("IDL:omg.org/CORBA/Current:1.0", "Current");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Current> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_Current); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Current>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::Current>(base); return !base || value; }

    class DomainManager : public virtual ::CORBA::Object {
        public:

        virtual ~DomainManager() = default;

        virtual ::omg::types::ref_type<::CORBA::Policy> get_domain_policy(::CORBA::PolicyType policy_type) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/DomainManager:1.0" || ::CORBA::Object::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/DomainManager:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/DomainManager:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_DomainManager = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/DomainManager:1.0", "DomainManager");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::DomainManager> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_DomainManager); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::DomainManager>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::DomainManager>(base); return !base || value; }

    class PollableSet : public virtual ::CORBA::LocalObject {
        public:

        class NoPossiblePollable : public ::CORBA::UserException {
            public:
            NoPossiblePollable() : what_("NoPossiblePollable") {}
            NoPossiblePollable(const NoPossiblePollable&) = default;
            NoPossiblePollable(NoPossiblePollable&&) = default;
            explicit NoPossiblePollable(const char* what) : what_(what ? what : "") {}
            ~NoPossiblePollable() noexcept override = default;
            NoPossiblePollable& operator=(const NoPossiblePollable&) = default;
            NoPossiblePollable& operator=(NoPossiblePollable&&) = default;

            void raise() const override { throw *this; }
            const char* _name() const override { return "NoPossiblePollable"; }
            const char* _rep_id() const override { return "IDL:omg.org/CORBA/PollableSet/NoPossiblePollable:1.0"; }
            void _insert_into(::CORBA::Any& any) const override;

            const char* what() const noexcept override { return what_.c_str(); }

            private:
            std::string what_;
        };
        #if __cplusplus >= 201703L
        static inline const ::omg::types::ref_type<::CORBA::TypeCode> _tc_NoPossiblePollable = ::CORBA::TypeCode::create_exception_tc("IDL:omg.org/CORBA/PollableSet/NoPossiblePollable:1.0", "NoPossiblePollable", {});
        #else
        static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_NoPossiblePollable;
        #endif
        friend void operator<<=(::CORBA::Any& any, const ::CORBA::PollableSet::NoPossiblePollable& value) { any._insert<::CORBA::PollableSet::NoPossiblePollable>(value, _tc_NoPossiblePollable); }
        friend void operator<<=(::CORBA::Any& any, ::CORBA::PollableSet::NoPossiblePollable&& value) { any._insert<::CORBA::PollableSet::NoPossiblePollable>(std::move(value), _tc_NoPossiblePollable); }
        friend bool operator>>=(const ::CORBA::Any& any, ::CORBA::PollableSet::NoPossiblePollable& value) { return any._extract<::CORBA::PollableSet::NoPossiblePollable>(value); }

        class UnknownPollable : public ::CORBA::UserException {
            public:
            UnknownPollable() : what_("UnknownPollable") {}
            UnknownPollable(const UnknownPollable&) = default;
            UnknownPollable(UnknownPollable&&) = default;
            explicit UnknownPollable(const char* what) : what_(what ? what : "") {}
            ~UnknownPollable() noexcept override = default;
            UnknownPollable& operator=(const UnknownPollable&) = default;
            UnknownPollable& operator=(UnknownPollable&&) = default;

            void raise() const override { throw *this; }
            const char* _name() const override { return "UnknownPollable"; }
            const char* _rep_id() const override { return "IDL:omg.org/CORBA/PollableSet/UnknownPollable:1.0"; }
            void _insert_into(::CORBA::Any& any) const override;

            const char* what() const noexcept override { return what_.c_str(); }

            private:
            std::string what_;
        };
        #if __cplusplus >= 201703L
        static inline const ::omg::types::ref_type<::CORBA::TypeCode> _tc_UnknownPollable = ::CORBA::TypeCode::create_exception_tc("IDL:omg.org/CORBA/PollableSet/UnknownPollable:1.0", "UnknownPollable", {});
        #else
        static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_UnknownPollable;
        #endif
        friend void operator<<=(::CORBA::Any& any, const ::CORBA::PollableSet::UnknownPollable& value) { any._insert<::CORBA::PollableSet::UnknownPollable>(value, _tc_UnknownPollable); }
        friend void operator<<=(::CORBA::Any& any, ::CORBA::PollableSet::UnknownPollable&& value) { any._insert<::CORBA::PollableSet::UnknownPollable>(std::move(value), _tc_UnknownPollable); }
        friend bool operator>>=(const ::CORBA::Any& any, ::CORBA::PollableSet::UnknownPollable& value) { return any._extract<::CORBA::PollableSet::UnknownPollable>(value); }

        virtual ~PollableSet() = default;

        virtual ::omg::types::ref_type<::CORBA::DIIPollable> create_dii_pollable() = 0;

        virtual void add_pollable(::omg::types::ref_type<::CORBA::Pollable> potential) = 0;

        virtual ::omg::types::ref_type<::CORBA::Pollable> get_ready_pollable(std::uint32_t timeout) = 0;

        virtual void remove(::omg::types::ref_type<::CORBA::Pollable> potential) = 0;

        virtual std::uint16_t number_left() = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/PollableSet:1.0" || ::CORBA::LocalObject::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/PollableSet:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/PollableSet:1.0"; }
    };
    #if __cplusplus < 201703L
    const ::omg::types::ref_type<::CORBA::TypeCode> CORBA::PollableSet::_tc_NoPossiblePollable = ::CORBA::TypeCode::create_exception_tc("IDL:omg.org/CORBA/PollableSet/NoPossiblePollable:1.0", "NoPossiblePollable", {});
    #endif
    inline void ::CORBA::PollableSet::NoPossiblePollable::_insert_into(::CORBA::Any& any) const { any <<= *this; }
    #if __cplusplus < 201703L
    const ::omg::types::ref_type<::CORBA::TypeCode> CORBA::PollableSet::_tc_UnknownPollable = ::CORBA::TypeCode::create_exception_tc("IDL:omg.org/CORBA/PollableSet/UnknownPollable:1.0", "UnknownPollable", {});
    #endif
    inline void ::CORBA::PollableSet::UnknownPollable::_insert_into(::CORBA::Any& any) const { any <<= *this; }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_PollableSet = ::CORBA::TypeCode::create_local_interface_tc("IDL:omg.org/CORBA/PollableSet:1.0", "PollableSet");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::PollableSet> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_PollableSet); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::PollableSet>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::PollableSet>(base); return !base || value; }

    class CustomMarshal : public virtual ::CORBA::ValueBase {
        public:
        virtual void marshal(::omg::types::ref_type<::CORBA::DataOutputStream> os) = 0;
        virtual void unmarshal(::omg::types::ref_type<::CORBA::DataInputStream> is) = 0;

        protected:
        CustomMarshal() = default;
        CustomMarshal(const CustomMarshal&) = default;
        CustomMarshal(CustomMarshal&&) = default;
        ~CustomMarshal() override = default;

        private:
        CustomMarshal& operator=(const CustomMarshal&) = delete;
        CustomMarshal& operator=(CustomMarshal&&) = delete;
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_CustomMarshal = ::CORBA::TypeCode::create_value_tc("IDL:omg.org/CORBA/CustomMarshal:1.0", "CustomMarshal", ::CORBA::VM_ABSTRACT, nullptr, {});
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::CustomMarshal> value) { any._insert<::omg::types::ref_type<::CORBA::ValueBase>>(std::move(value), _tc_CustomMarshal); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::CustomMarshal>& value) { ::omg::types::ref_type<::CORBA::ValueBase> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::CustomMarshal>(base); return !base || value; }

    using AnySeq = ::omg::types::sequence<::CORBA::Any>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_AnySeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/AnySeq:1.0", "AnySeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_any));

    using BooleanSeq = ::omg::types::sequence<bool>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_BooleanSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/BooleanSeq:1.0", "BooleanSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_boolean));

    using CharSeq = ::omg::types::sequence<char>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_CharSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/CharSeq:1.0", "CharSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_char));

    using WCharSeq = ::omg::types::sequence<wchar_t>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_WCharSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/WCharSeq:1.0", "WCharSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_wchar));

    using OctetSeq = ::omg::types::sequence<std::uint8_t>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_OctetSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/OctetSeq:1.0", "OctetSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_octet));

    using ShortSeq = ::omg::types::sequence<std::int16_t>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ShortSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ShortSeq:1.0", "ShortSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_short));

    using UShortSeq = ::omg::types::sequence<std::uint16_t>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_UShortSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/UShortSeq:1.0", "UShortSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_ushort));

    using LongSeq = ::omg::types::sequence<std::int32_t>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_LongSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/LongSeq:1.0", "LongSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_long));

    using ULongSeq = ::omg::types::sequence<std::uint32_t>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ULongSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ULongSeq:1.0", "ULongSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_ulong));

    using LongLongSeq = ::omg::types::sequence<std::int64_t>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_LongLongSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/LongLongSeq:1.0", "LongLongSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_longlong));

    using ULongLongSeq = ::omg::types::sequence<std::uint64_t>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ULongLongSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ULongLongSeq:1.0", "ULongLongSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_ulonglong));

    using FloatSeq = ::omg::types::sequence<float>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_FloatSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/FloatSeq:1.0", "FloatSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_float));

    using DoubleSeq = ::omg::types::sequence<double>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_DoubleSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/DoubleSeq:1.0", "DoubleSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_double));

    class DataInputStream : public virtual ::CORBA::ValueBase {
        public:
        virtual ::CORBA::Any read_any() = 0;
        virtual bool read_boolean() = 0;
        virtual char read_char() = 0;
        virtual wchar_t read_wchar() = 0;
        virtual std::uint8_t read_octet() = 0;
        virtual std::int16_t read_short() = 0;
        virtual std::uint16_t read_ushort() = 0;
        virtual std::int32_t read_long() = 0;
        virtual std::uint32_t read_ulong() = 0;
        virtual std::int64_t read_longlong() = 0;
        virtual std::uint64_t read_ulonglong() = 0;
        virtual float read_float() = 0;
        virtual double read_double() = 0;
        virtual long double read_longdouble() = 0;
        virtual std::string read_string() = 0;
        virtual std::wstring read_wstring() = 0;
        virtual ::omg::types::ref_type<::CORBA::Object> read_Object() = 0;
        virtual ::omg::types::ref_type<::CORBA::AbstractBase> read_Abstract() = 0;
        virtual ::omg::types::ref_type<::CORBA::ValueBase> read_Value() = 0;
        virtual ::omg::types::ref_type<::CORBA::TypeCode> read_TypeCode() = 0;
        virtual void read_any_array(::CORBA::AnySeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_boolean_array(::CORBA::BooleanSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_char_array(::CORBA::CharSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_wchar_array(::CORBA::WCharSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_octet_array(::CORBA::OctetSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_short_array(::CORBA::ShortSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_ushort_array(::CORBA::UShortSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_long_array(::CORBA::LongSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_ulong_array(::CORBA::ULongSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_ulonglong_array(::CORBA::ULongLongSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_longlong_array(::CORBA::LongLongSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_float_array(::CORBA::FloatSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_double_array(::CORBA::DoubleSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void read_long_double_array(::CORBA::DoubleSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual ::CORBA::Any read_fixed(std::uint16_t digits, std::int16_t scale) = 0;
        virtual void read_fixed_array(::CORBA::AnySeq& seq, std::uint32_t offset, std::uint32_t length, std::uint16_t digits, std::int16_t scale) = 0;

        protected:
        DataInputStream() = default;
        DataInputStream(const DataInputStream&) = default;
        DataInputStream(DataInputStream&&) = default;
        ~DataInputStream() override = default;

        private:
        DataInputStream& operator=(const DataInputStream&) = delete;
        DataInputStream& operator=(DataInputStream&&) = delete;
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_DataInputStream = ::CORBA::TypeCode::create_value_tc("IDL:omg.org/CORBA/DataInputStream:1.0", "DataInputStream", ::CORBA::VM_ABSTRACT, nullptr, {});
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::DataInputStream> value) { any._insert<::omg::types::ref_type<::CORBA::ValueBase>>(std::move(value), _tc_DataInputStream); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::DataInputStream>& value) { ::omg::types::ref_type<::CORBA::ValueBase> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::DataInputStream>(base); return !base || value; }

    using LongDoubleSeq = ::omg::types::sequence<long double>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_LongDoubleSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/LongDoubleSeq:1.0", "LongDoubleSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_longdouble));

    class DataOutputStream : public virtual ::CORBA::ValueBase {
        public:
        virtual void write_any(const ::CORBA::Any& value) = 0;
        virtual void write_boolean(bool value) = 0;
        virtual void write_char(char value) = 0;
        virtual void write_wchar(wchar_t value) = 0;
        virtual void write_octet(std::uint8_t value) = 0;
        virtual void write_short(std::int16_t value) = 0;
        virtual void write_ushort(std::uint16_t value) = 0;
        virtual void write_long(std::int32_t value) = 0;
        virtual void write_ulong(std::uint32_t value) = 0;
        virtual void write_longlong(std::int64_t value) = 0;
        virtual void write_ulonglong(std::uint64_t value) = 0;
        virtual void write_float(float value) = 0;
        virtual void write_double(double value) = 0;
        virtual void write_longdouble(long double value) = 0;
        virtual void write_string(const std::string& value) = 0;
        virtual void write_wstring(const std::wstring& value) = 0;
        virtual void write_Object(::omg::types::ref_type<::CORBA::Object> value) = 0;
        virtual void write_Abstract(::omg::types::ref_type<::CORBA::AbstractBase> value) = 0;
        virtual void write_Value(::omg::types::ref_type<::CORBA::ValueBase> value) = 0;
        virtual void write_TypeCode(::omg::types::ref_type<::CORBA::TypeCode> value) = 0;
        virtual void write_any_array(const ::CORBA::AnySeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_boolean_array(const ::CORBA::BooleanSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_char_array(const ::CORBA::CharSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_wchar_array(const ::CORBA::WCharSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_octet_array(const ::CORBA::OctetSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_short_array(const ::CORBA::ShortSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_ushort_array(const ::CORBA::UShortSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_long_array(const ::CORBA::LongSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_ulong_array(const ::CORBA::ULongSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_ulonglong_array(const ::CORBA::ULongLongSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_longlong_array(const ::CORBA::LongLongSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_float_array(const ::CORBA::FloatSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_double_array(const ::CORBA::DoubleSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_long_double_array(const ::CORBA::LongDoubleSeq& seq, std::uint32_t offset, std::uint32_t length) = 0;
        virtual void write_fixed(const ::CORBA::Any& fixed_value) = 0;
        virtual void write_fixed_array(const ::CORBA::AnySeq& seq, std::uint32_t offset, std::uint32_t length) = 0;

        protected:
        DataOutputStream() = default;
        DataOutputStream(const DataOutputStream&) = default;
        DataOutputStream(DataOutputStream&&) = default;
        ~DataOutputStream() override = default;

        private:
        DataOutputStream& operator=(const DataOutputStream&) = delete;
        DataOutputStream& operator=(DataOutputStream&&) = delete;
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_DataOutputStream = ::CORBA::TypeCode::create_value_tc("IDL:omg.org/CORBA/DataOutputStream:1.0", "DataOutputStream", ::CORBA::VM_ABSTRACT, nullptr, {});
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::DataOutputStream> value) { any._insert<::omg::types::ref_type<::CORBA::ValueBase>>(std::move(value), _tc_DataOutputStream); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::DataOutputStream>& value) { ::omg::types::ref_type<::CORBA::ValueBase> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::DataOutputStream>(base); return !base || value; }

    enum class DefinitionKind {
        dk_none = 0,
        dk_all = 1,
        dk_Attribute = 2,
        dk_Constant = 3,
        dk_Exception = 4,
        dk_Interface = 5,
        dk_Module = 6,
        dk_Operation = 7,
        dk_Typedef = 8,
        dk_Alias = 9,
        dk_Struct = 10,
        dk_Union = 11,
        dk_Enum = 12,
        dk_Primitive = 13,
        dk_String = 14,
        dk_Sequence = 15,
        dk_Array = 16,
        dk_Repository = 17,
        dk_Wstring = 18,
        dk_Fixed = 19,
        dk_Value = 20,
        dk_ValueBox = 21,
        dk_ValueMember = 22,
        dk_Native = 23,
        dk_AbstractInterface = 24,
        dk_LocalInterface = 25,
        dk_Component = 26,
        dk_Home = 27,
        dk_Factory = 28,
        dk_Finder = 29,
        dk_Emits = 30,
        dk_Publishes = 31,
        dk_Consumes = 32,
        dk_Provides = 33,
        dk_Uses = 34,
        dk_Event = 35
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_DefinitionKind = ::CORBA::TypeCode::create_enum_tc("IDL:omg.org/CORBA/DefinitionKind:1.0", "DefinitionKind", {"dk_none", "dk_all", "dk_Attribute", "dk_Constant", "dk_Exception", "dk_Interface", "dk_Module", "dk_Operation", "dk_Typedef", "dk_Alias", "dk_Struct", "dk_Union", "dk_Enum", "dk_Primitive", "dk_String", "dk_Sequence", "dk_Array", "dk_Repository", "dk_Wstring", "dk_Fixed", "dk_Value", "dk_ValueBox", "dk_ValueMember", "dk_Native", "dk_AbstractInterface", "dk_LocalInterface", "dk_Component", "dk_Home", "dk_Factory", "dk_Finder", "dk_Emits", "dk_Publishes", "dk_Consumes", "dk_Provides", "dk_Uses", "dk_Event"});
    inline void operator<<=(::CORBA::Any& any, ::CORBA::DefinitionKind value) { any._insert<::CORBA::DefinitionKind>(value, _tc_DefinitionKind); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::DefinitionKind& value) { return any._extract<::CORBA::DefinitionKind>(value); }

    class IRObject : public virtual ::CORBA::Object {
        public:

        virtual ~IRObject() = default;

        virtual ::CORBA::DefinitionKind def_kind() const = 0;

        virtual void destroy() = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/IRObject:1.0" || ::CORBA::Object::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/IRObject:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/IRObject:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_IRObject = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/IRObject:1.0", "IRObject");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::IRObject> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_IRObject); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::IRObject>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::IRObject>(base); return !base || value; }

    using Identifier = std::string;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Identifier = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/Identifier:1.0", "Identifier", ::CORBA::TypeCode::create_string_tc(0));

    using ScopedName = std::string;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ScopedName = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ScopedName:1.0", "ScopedName", ::CORBA::TypeCode::create_string_tc(0));

    using RepositoryId = std::string;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_RepositoryId = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/RepositoryId:1.0", "RepositoryId", ::CORBA::TypeCode::create_string_tc(0));

    using VersionSpec = std::string;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_VersionSpec = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/VersionSpec:1.0", "VersionSpec", ::CORBA::TypeCode::create_string_tc(0));

    class Contained : public virtual ::CORBA::IRObject {
        public:

        struct Description {
            ::CORBA::DefinitionKind kind {::CORBA::DefinitionKind::dk_none};
            ::CORBA::Any value;
        };
        friend bool operator==(const Description& a, const Description& b) {
            return a.kind == b.kind
                && a.value == b.value;
        }
        friend bool operator!=(const Description& a, const Description& b) {
            return !(a == b);
        }
        friend void swap(Description& a, Description& b) {
            using std::swap;
            swap(a.kind, b.kind);
            swap(a.value, b.value);
        }
        #if __cplusplus >= 201703L
        static inline const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Description = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/Contained/Description:1.0", "Description", {::CORBA::TypeCode::Member{"kind", ::CORBA::_tc_DefinitionKind, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"value", ::CORBA::_tc_any, nullptr, ::CORBA::PUBLIC_MEMBER}});
        #else
        static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Description;
        #endif
        friend void operator<<=(::CORBA::Any& any, const ::CORBA::Contained::Description& value) { any._insert<::CORBA::Contained::Description>(value, _tc_Description); }
        friend void operator<<=(::CORBA::Any& any, ::CORBA::Contained::Description&& value) { any._insert<::CORBA::Contained::Description>(std::move(value), _tc_Description); }
        friend bool operator>>=(const ::CORBA::Any& any, ::CORBA::Contained::Description& value) { return any._extract<::CORBA::Contained::Description>(value); }

        virtual ~Contained() = default;

        virtual void id(const ::CORBA::RepositoryId& value) = 0;
        virtual ::CORBA::RepositoryId id() const = 0;

        virtual void name(const ::CORBA::Identifier& value) = 0;
        virtual ::CORBA::Identifier name() const = 0;

        virtual void version(const ::CORBA::VersionSpec& value) = 0;
        virtual ::CORBA::VersionSpec version() const = 0;

        virtual ::omg::types::ref_type<::CORBA::Container> defined_in() const = 0;

        virtual ::CORBA::ScopedName absolute_name() const = 0;

        virtual ::omg::types::ref_type<::CORBA::Repository> containing_repository() const = 0;

        virtual ::CORBA::Contained::Description describe() = 0;

        virtual void move(::omg::types::ref_type<::CORBA::Container> new_container, const ::CORBA::Identifier& new_name, const ::CORBA::VersionSpec& new_version) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/Contained:1.0" || ::CORBA::IRObject::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/Contained:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/Contained:1.0"; }
    };
    #if __cplusplus < 201703L
    const ::omg::types::ref_type<::CORBA::TypeCode> CORBA::Contained::_tc_Description = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/Contained/Description:1.0", "Description", {::CORBA::TypeCode::Member{"kind", ::CORBA::_tc_DefinitionKind, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"value", ::CORBA::_tc_any, nullptr, ::CORBA::PUBLIC_MEMBER}});
    #endif

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Contained = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/Contained:1.0", "Contained");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Contained> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_Contained); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Contained>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::Contained>(base); return !base || value; }

    using InterfaceDefSeq = ::omg::types::sequence<::omg::types::ref_type<::CORBA::InterfaceDef>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_InterfaceDefSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/InterfaceDefSeq:1.0", "InterfaceDefSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/InterfaceDef:1.0", "InterfaceDef")));

    using ValueDefSeq = ::omg::types::sequence<::omg::types::ref_type<::CORBA::ValueDef>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ValueDefSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ValueDefSeq:1.0", "ValueDefSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ValueDef:1.0", "ValueDef")));

    using AbstractInterfaceDefSeq = ::omg::types::sequence<::omg::types::ref_type<::CORBA::AbstractInterfaceDef>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_AbstractInterfaceDefSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/AbstractInterfaceDefSeq:1.0", "AbstractInterfaceDefSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/AbstractInterfaceDef:1.0", "AbstractInterfaceDef")));

    using ContainedSeq = ::omg::types::sequence<::omg::types::ref_type<::CORBA::Contained>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ContainedSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ContainedSeq:1.0", "ContainedSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/Contained:1.0", "Contained")));

    using StructMemberSeq = ::omg::types::sequence<::CORBA::StructMember>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_StructMemberSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/StructMemberSeq:1.0", "StructMemberSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_recursive_tc("IDL:omg.org/CORBA/StructMember:1.0")));

    using InitializerSeq = ::omg::types::sequence<::CORBA::Initializer>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_InitializerSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/InitializerSeq:1.0", "InitializerSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_recursive_tc("IDL:omg.org/CORBA/Initializer:1.0")));

    using ExtInitializerSeq = ::omg::types::sequence<::CORBA::ExtInitializer>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtInitializerSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ExtInitializerSeq:1.0", "ExtInitializerSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_recursive_tc("IDL:omg.org/CORBA/ExtInitializer:1.0")));

    using UnionMemberSeq = ::omg::types::sequence<::CORBA::UnionMember>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_UnionMemberSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/UnionMemberSeq:1.0", "UnionMemberSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_recursive_tc("IDL:omg.org/CORBA/UnionMember:1.0")));

    using EnumMemberSeq = ::omg::types::sequence<::CORBA::Identifier>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_EnumMemberSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/EnumMemberSeq:1.0", "EnumMemberSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_Identifier));

    class Container : public virtual ::CORBA::IRObject {
        public:

        struct Description {
            ::omg::types::ref_type<::CORBA::Contained> contained_object;
            ::CORBA::DefinitionKind kind {::CORBA::DefinitionKind::dk_none};
            ::CORBA::Any value;
        };
        friend bool operator==(const Description& a, const Description& b) {
            return a.contained_object == b.contained_object
                && a.kind == b.kind
                && a.value == b.value;
        }
        friend bool operator!=(const Description& a, const Description& b) {
            return !(a == b);
        }
        friend void swap(Description& a, Description& b) {
            using std::swap;
            swap(a.contained_object, b.contained_object);
            swap(a.kind, b.kind);
            swap(a.value, b.value);
        }
        #if __cplusplus >= 201703L
        static inline const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Description = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/Container/Description:1.0", "Description", {::CORBA::TypeCode::Member{"contained_object", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/Contained:1.0", "Contained"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"kind", ::CORBA::_tc_DefinitionKind, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"value", ::CORBA::_tc_any, nullptr, ::CORBA::PUBLIC_MEMBER}});
        #else
        static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Description;
        #endif
        friend void operator<<=(::CORBA::Any& any, const ::CORBA::Container::Description& value) { any._insert<::CORBA::Container::Description>(value, _tc_Description); }
        friend void operator<<=(::CORBA::Any& any, ::CORBA::Container::Description&& value) { any._insert<::CORBA::Container::Description>(std::move(value), _tc_Description); }
        friend bool operator>>=(const ::CORBA::Any& any, ::CORBA::Container::Description& value) { return any._extract<::CORBA::Container::Description>(value); }

        using DescriptionSeq = ::omg::types::sequence<::CORBA::Container::Description>;
        #if __cplusplus >= 201703L
        static inline const ::omg::types::ref_type<::CORBA::TypeCode> _tc_DescriptionSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/Container/DescriptionSeq:1.0", "DescriptionSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::Container::_tc_Description));
        #else
        static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_DescriptionSeq;
        #endif

        virtual ~Container() = default;

        virtual ::omg::types::ref_type<::CORBA::Contained> lookup(const ::CORBA::ScopedName& search_name) = 0;

        virtual ::CORBA::ContainedSeq contents(::CORBA::DefinitionKind limit_type, bool exclude_inherited) = 0;

        virtual ::CORBA::ContainedSeq lookup_name(const ::CORBA::Identifier& search_name, std::int32_t levels_to_search, ::CORBA::DefinitionKind limit_type, bool exclude_inherited) = 0;

        virtual ::CORBA::Container::DescriptionSeq describe_contents(::CORBA::DefinitionKind limit_type, bool exclude_inherited, std::int32_t max_returned_objs) = 0;

        virtual ::omg::types::ref_type<::CORBA::ModuleDef> create_module(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version) = 0;

        virtual ::omg::types::ref_type<::CORBA::ConstantDef> create_constant(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, ::omg::types::ref_type<::CORBA::IDLType> type, const ::CORBA::Any& value) = 0;

        virtual ::omg::types::ref_type<::CORBA::StructDef> create_struct(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, const ::CORBA::StructMemberSeq& members) = 0;

        virtual ::omg::types::ref_type<::CORBA::UnionDef> create_union(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, ::omg::types::ref_type<::CORBA::IDLType> discriminator_type, const ::CORBA::UnionMemberSeq& members) = 0;

        virtual ::omg::types::ref_type<::CORBA::EnumDef> create_enum(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, const ::CORBA::EnumMemberSeq& members) = 0;

        virtual ::omg::types::ref_type<::CORBA::AliasDef> create_alias(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, ::omg::types::ref_type<::CORBA::IDLType> original_type) = 0;

        virtual ::omg::types::ref_type<::CORBA::InterfaceDef> create_interface(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, const ::CORBA::InterfaceDefSeq& base_interfaces, bool is_abstract) = 0;

        virtual ::omg::types::ref_type<::CORBA::ValueDef> create_value(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, bool is_custom, bool is_abstract, ::omg::types::ref_type<::CORBA::ValueDef> base_value, bool is_truncatable, const ::CORBA::ValueDefSeq& abstract_base_values, const ::CORBA::InterfaceDefSeq& supported_interfaces, const ::CORBA::InitializerSeq& initializers) = 0;

        virtual ::omg::types::ref_type<::CORBA::ValueBoxDef> create_value_box(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, ::omg::types::ref_type<::CORBA::IDLType> original_type_def) = 0;

        virtual ::omg::types::ref_type<::CORBA::ExceptionDef> create_exception(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, const ::CORBA::StructMemberSeq& members) = 0;

        virtual ::omg::types::ref_type<::CORBA::NativeDef> create_native(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version) = 0;

        virtual ::omg::types::ref_type<::CORBA::AbstractInterfaceDef> create_abstract_interface(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, const ::CORBA::AbstractInterfaceDefSeq& base_interfaces) = 0;

        virtual ::omg::types::ref_type<::CORBA::LocalInterfaceDef> create_local_interface(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, const ::CORBA::InterfaceDefSeq& base_interfaces) = 0;

        virtual ::omg::types::ref_type<::CORBA::ExtValueDef> create_ext_value(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, bool is_custom, bool is_abstract, ::omg::types::ref_type<::CORBA::ValueDef> base_value, bool is_truncatable, const ::CORBA::ValueDefSeq& abstract_base_values, const ::CORBA::InterfaceDefSeq& supported_interfaces, const ::CORBA::ExtInitializerSeq& initializers) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/Container:1.0" || ::CORBA::IRObject::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/Container:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/Container:1.0"; }
    };
    #if __cplusplus < 201703L
    const ::omg::types::ref_type<::CORBA::TypeCode> CORBA::Container::_tc_Description = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/Container/Description:1.0", "Description", {::CORBA::TypeCode::Member{"contained_object", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/Contained:1.0", "Contained"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"kind", ::CORBA::_tc_DefinitionKind, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"value", ::CORBA::_tc_any, nullptr, ::CORBA::PUBLIC_MEMBER}});
    #endif
    #if __cplusplus < 201703L
    const ::omg::types::ref_type<::CORBA::TypeCode> CORBA::Container::_tc_DescriptionSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/Container/DescriptionSeq:1.0", "DescriptionSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::Container::_tc_Description));
    #endif

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Container = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/Container:1.0", "Container");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Container> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_Container); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Container>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::Container>(base); return !base || value; }

    class IDLType : public virtual ::CORBA::IRObject {
        public:

        virtual ~IDLType() = default;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> type() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/IDLType:1.0" || ::CORBA::IRObject::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/IDLType:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/IDLType:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_IDLType = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/IDLType:1.0", "IDLType");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::IDLType> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_IDLType); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::IDLType>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::IDLType>(base); return !base || value; }

    enum class AttributeMode {
        ATTR_NORMAL = 0,
        ATTR_READONLY = 1
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_AttributeMode = ::CORBA::TypeCode::create_enum_tc("IDL:omg.org/CORBA/AttributeMode:1.0", "AttributeMode", {"ATTR_NORMAL", "ATTR_READONLY"});
    inline void operator<<=(::CORBA::Any& any, ::CORBA::AttributeMode value) { any._insert<::CORBA::AttributeMode>(value, _tc_AttributeMode); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::AttributeMode& value) { return any._extract<::CORBA::AttributeMode>(value); }

    enum class OperationMode {
        OP_NORMAL = 0,
        OP_ONEWAY = 1
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_OperationMode = ::CORBA::TypeCode::create_enum_tc("IDL:omg.org/CORBA/OperationMode:1.0", "OperationMode", {"OP_NORMAL", "OP_ONEWAY"});
    inline void operator<<=(::CORBA::Any& any, ::CORBA::OperationMode value) { any._insert<::CORBA::OperationMode>(value, _tc_OperationMode); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::OperationMode& value) { return any._extract<::CORBA::OperationMode>(value); }

    using ParDescriptionSeq = ::omg::types::sequence<::CORBA::ParameterDescription>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ParDescriptionSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ParDescriptionSeq:1.0", "ParDescriptionSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_recursive_tc("IDL:omg.org/CORBA/ParameterDescription:1.0")));

    using ContextIdentifier = ::CORBA::Identifier;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ContextIdentifier = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ContextIdentifier:1.0", "ContextIdentifier", ::CORBA::_tc_Identifier);

    using ContextIdSeq = ::omg::types::sequence<::CORBA::ContextIdentifier>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ContextIdSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ContextIdSeq:1.0", "ContextIdSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_ContextIdentifier));

    using ExceptionDefSeq = ::omg::types::sequence<::omg::types::ref_type<::CORBA::ExceptionDef>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExceptionDefSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ExceptionDefSeq:1.0", "ExceptionDefSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ExceptionDef:1.0", "ExceptionDef")));

    using RepositoryIdSeq = ::omg::types::sequence<::CORBA::RepositoryId>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_RepositoryIdSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/RepositoryIdSeq:1.0", "RepositoryIdSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_RepositoryId));

    using OpDescriptionSeq = ::omg::types::sequence<::CORBA::OperationDescription>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_OpDescriptionSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/OpDescriptionSeq:1.0", "OpDescriptionSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_recursive_tc("IDL:omg.org/CORBA/OperationDescription:1.0")));

    using AttrDescriptionSeq = ::omg::types::sequence<::CORBA::AttributeDescription>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_AttrDescriptionSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/AttrDescriptionSeq:1.0", "AttrDescriptionSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_recursive_tc("IDL:omg.org/CORBA/AttributeDescription:1.0")));

    class InterfaceDef : public virtual ::CORBA::Container, public virtual ::CORBA::Contained, public virtual ::CORBA::IDLType {
        public:

        struct FullInterfaceDescription {
            ::CORBA::Identifier name;
            ::CORBA::RepositoryId id;
            ::CORBA::RepositoryId defined_in;
            ::CORBA::VersionSpec version;
            ::CORBA::OpDescriptionSeq operations;
            ::CORBA::AttrDescriptionSeq attributes;
            ::CORBA::RepositoryIdSeq base_interfaces;
            ::omg::types::ref_type<::CORBA::TypeCode> type;
            bool is_abstract {};
        };
        friend bool operator==(const FullInterfaceDescription& a, const FullInterfaceDescription& b) {
            return a.name == b.name
                && a.id == b.id
                && a.defined_in == b.defined_in
                && a.version == b.version
                && a.operations == b.operations
                && a.attributes == b.attributes
                && a.base_interfaces == b.base_interfaces
                && a.type == b.type
                && a.is_abstract == b.is_abstract;
        }
        friend bool operator!=(const FullInterfaceDescription& a, const FullInterfaceDescription& b) {
            return !(a == b);
        }
        friend void swap(FullInterfaceDescription& a, FullInterfaceDescription& b) {
            using std::swap;
            swap(a.name, b.name);
            swap(a.id, b.id);
            swap(a.defined_in, b.defined_in);
            swap(a.version, b.version);
            swap(a.operations, b.operations);
            swap(a.attributes, b.attributes);
            swap(a.base_interfaces, b.base_interfaces);
            swap(a.type, b.type);
            swap(a.is_abstract, b.is_abstract);
        }
        #if __cplusplus >= 201703L
        static inline const ::omg::types::ref_type<::CORBA::TypeCode> _tc_FullInterfaceDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/InterfaceDef/FullInterfaceDescription:1.0", "FullInterfaceDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"operations", ::CORBA::_tc_OpDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"attributes", ::CORBA::_tc_AttrDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"base_interfaces", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_abstract", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}});
        #else
        static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_FullInterfaceDescription;
        #endif
        friend void operator<<=(::CORBA::Any& any, const ::CORBA::InterfaceDef::FullInterfaceDescription& value) { any._insert<::CORBA::InterfaceDef::FullInterfaceDescription>(value, _tc_FullInterfaceDescription); }
        friend void operator<<=(::CORBA::Any& any, ::CORBA::InterfaceDef::FullInterfaceDescription&& value) { any._insert<::CORBA::InterfaceDef::FullInterfaceDescription>(std::move(value), _tc_FullInterfaceDescription); }
        friend bool operator>>=(const ::CORBA::Any& any, ::CORBA::InterfaceDef::FullInterfaceDescription& value) { return any._extract<::CORBA::InterfaceDef::FullInterfaceDescription>(value); }

        virtual ~InterfaceDef() = default;

        virtual void base_interfaces(const ::CORBA::InterfaceDefSeq& value) = 0;
        virtual ::CORBA::InterfaceDefSeq base_interfaces() const = 0;

        virtual void is_abstract(bool value) = 0;
        virtual bool is_abstract() const = 0;

        virtual bool is_a(const ::CORBA::RepositoryId& interface_id) = 0;

        virtual ::CORBA::InterfaceDef::FullInterfaceDescription describe_interface() = 0;

        virtual ::omg::types::ref_type<::CORBA::AttributeDef> create_attribute(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, ::omg::types::ref_type<::CORBA::IDLType> type, ::CORBA::AttributeMode mode) = 0;

        virtual ::omg::types::ref_type<::CORBA::OperationDef> create_operation(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, ::omg::types::ref_type<::CORBA::IDLType> result, ::CORBA::OperationMode mode, const ::CORBA::ParDescriptionSeq& params, const ::CORBA::ExceptionDefSeq& exceptions, const ::CORBA::ContextIdSeq& contexts) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/InterfaceDef:1.0" || ::CORBA::Container::_is_a(logical_type_id) || ::CORBA::Contained::_is_a(logical_type_id) || ::CORBA::IDLType::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/InterfaceDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/InterfaceDef:1.0"; }
    };
    #if __cplusplus < 201703L
    const ::omg::types::ref_type<::CORBA::TypeCode> CORBA::InterfaceDef::_tc_FullInterfaceDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/InterfaceDef/FullInterfaceDescription:1.0", "FullInterfaceDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"operations", ::CORBA::_tc_OpDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"attributes", ::CORBA::_tc_AttrDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"base_interfaces", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_abstract", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}});
    #endif

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_InterfaceDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/InterfaceDef:1.0", "InterfaceDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::InterfaceDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_InterfaceDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::InterfaceDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::InterfaceDef>(base); return !base || value; }

    class AbstractInterfaceDef : public virtual ::CORBA::InterfaceDef {
        public:

        virtual ~AbstractInterfaceDef() = default;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/AbstractInterfaceDef:1.0" || ::CORBA::InterfaceDef::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/AbstractInterfaceDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/AbstractInterfaceDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_AbstractInterfaceDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/AbstractInterfaceDef:1.0", "AbstractInterfaceDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::AbstractInterfaceDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_AbstractInterfaceDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::AbstractInterfaceDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::AbstractInterfaceDef>(base); return !base || value; }

    class TypedefDef : public virtual ::CORBA::Contained, public virtual ::CORBA::IDLType {
        public:

        virtual ~TypedefDef() = default;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/TypedefDef:1.0" || ::CORBA::Contained::_is_a(logical_type_id) || ::CORBA::IDLType::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/TypedefDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/TypedefDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_TypedefDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypedefDef:1.0", "TypedefDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::TypedefDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_TypedefDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::TypedefDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::TypedefDef>(base); return !base || value; }

    class AliasDef : public virtual ::CORBA::TypedefDef {
        public:

        virtual ~AliasDef() = default;

        virtual void original_type_def(::omg::types::ref_type<::CORBA::IDLType> value) = 0;
        virtual ::omg::types::ref_type<::CORBA::IDLType> original_type_def() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/AliasDef:1.0" || ::CORBA::TypedefDef::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/AliasDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/AliasDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_AliasDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/AliasDef:1.0", "AliasDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::AliasDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_AliasDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::AliasDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::AliasDef>(base); return !base || value; }

    class ArrayDef : public virtual ::CORBA::IDLType {
        public:

        virtual ~ArrayDef() = default;

        virtual void length(std::uint32_t value) = 0;
        virtual std::uint32_t length() const = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> element_type() const = 0;

        virtual void element_type_def(::omg::types::ref_type<::CORBA::IDLType> value) = 0;
        virtual ::omg::types::ref_type<::CORBA::IDLType> element_type_def() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ArrayDef:1.0" || ::CORBA::IDLType::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ArrayDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ArrayDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ArrayDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ArrayDef:1.0", "ArrayDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ArrayDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ArrayDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ArrayDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ArrayDef>(base); return !base || value; }

    class AttributeDef : public virtual ::CORBA::Contained {
        public:

        virtual ~AttributeDef() = default;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> type() const = 0;

        virtual void type_def(::omg::types::ref_type<::CORBA::IDLType> value) = 0;
        virtual ::omg::types::ref_type<::CORBA::IDLType> type_def() const = 0;

        virtual void mode(::CORBA::AttributeMode value) = 0;
        virtual ::CORBA::AttributeMode mode() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/AttributeDef:1.0" || ::CORBA::Contained::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/AttributeDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/AttributeDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_AttributeDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/AttributeDef:1.0", "AttributeDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::AttributeDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_AttributeDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::AttributeDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::AttributeDef>(base); return !base || value; }

    class ConstantDef : public virtual ::CORBA::Contained {
        public:

        virtual ~ConstantDef() = default;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> type() const = 0;

        virtual void type_def(::omg::types::ref_type<::CORBA::IDLType> value) = 0;
        virtual ::omg::types::ref_type<::CORBA::IDLType> type_def() const = 0;

        virtual void value(const ::CORBA::Any& value) = 0;
        virtual ::CORBA::Any value() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ConstantDef:1.0" || ::CORBA::Contained::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ConstantDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ConstantDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ConstantDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ConstantDef:1.0", "ConstantDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ConstantDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ConstantDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ConstantDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ConstantDef>(base); return !base || value; }

    class EnumDef : public virtual ::CORBA::TypedefDef {
        public:

        virtual ~EnumDef() = default;

        virtual void members(const ::CORBA::EnumMemberSeq& value) = 0;
        virtual ::CORBA::EnumMemberSeq members() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/EnumDef:1.0" || ::CORBA::TypedefDef::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/EnumDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/EnumDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_EnumDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/EnumDef:1.0", "EnumDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::EnumDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_EnumDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::EnumDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::EnumDef>(base); return !base || value; }

    class ExceptionDef : public virtual ::CORBA::Contained, public virtual ::CORBA::Container {
        public:

        virtual ~ExceptionDef() = default;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> type() const = 0;

        virtual void members(const ::CORBA::StructMemberSeq& value) = 0;
        virtual ::CORBA::StructMemberSeq members() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ExceptionDef:1.0" || ::CORBA::Contained::_is_a(logical_type_id) || ::CORBA::Container::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ExceptionDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ExceptionDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExceptionDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ExceptionDef:1.0", "ExceptionDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ExceptionDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ExceptionDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ExceptionDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ExceptionDef>(base); return !base || value; }

    using ExtAttrDescriptionSeq = ::omg::types::sequence<::CORBA::ExtAttributeDescription>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtAttrDescriptionSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ExtAttrDescriptionSeq:1.0", "ExtAttrDescriptionSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_recursive_tc("IDL:omg.org/CORBA/ExtAttributeDescription:1.0")));

    class InterfaceAttrExtension : public virtual ::CORBA::Object {
        public:

        struct ExtFullInterfaceDescription {
            ::CORBA::Identifier name;
            ::CORBA::RepositoryId id;
            ::CORBA::RepositoryId defined_in;
            ::CORBA::VersionSpec version;
            ::CORBA::OpDescriptionSeq operations;
            ::CORBA::ExtAttrDescriptionSeq attributes;
            ::CORBA::RepositoryIdSeq base_interfaces;
            ::omg::types::ref_type<::CORBA::TypeCode> type;
        };
        friend bool operator==(const ExtFullInterfaceDescription& a, const ExtFullInterfaceDescription& b) {
            return a.name == b.name
                && a.id == b.id
                && a.defined_in == b.defined_in
                && a.version == b.version
                && a.operations == b.operations
                && a.attributes == b.attributes
                && a.base_interfaces == b.base_interfaces
                && a.type == b.type;
        }
        friend bool operator!=(const ExtFullInterfaceDescription& a, const ExtFullInterfaceDescription& b) {
            return !(a == b);
        }
        friend void swap(ExtFullInterfaceDescription& a, ExtFullInterfaceDescription& b) {
            using std::swap;
            swap(a.name, b.name);
            swap(a.id, b.id);
            swap(a.defined_in, b.defined_in);
            swap(a.version, b.version);
            swap(a.operations, b.operations);
            swap(a.attributes, b.attributes);
            swap(a.base_interfaces, b.base_interfaces);
            swap(a.type, b.type);
        }
        #if __cplusplus >= 201703L
        static inline const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtFullInterfaceDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/InterfaceAttrExtension/ExtFullInterfaceDescription:1.0", "ExtFullInterfaceDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"operations", ::CORBA::_tc_OpDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"attributes", ::CORBA::_tc_ExtAttrDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"base_interfaces", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}});
        #else
        static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtFullInterfaceDescription;
        #endif
        friend void operator<<=(::CORBA::Any& any, const ::CORBA::InterfaceAttrExtension::ExtFullInterfaceDescription& value) { any._insert<::CORBA::InterfaceAttrExtension::ExtFullInterfaceDescription>(value, _tc_ExtFullInterfaceDescription); }
        friend void operator<<=(::CORBA::Any& any, ::CORBA::InterfaceAttrExtension::ExtFullInterfaceDescription&& value) { any._insert<::CORBA::InterfaceAttrExtension::ExtFullInterfaceDescription>(std::move(value), _tc_ExtFullInterfaceDescription); }
        friend bool operator>>=(const ::CORBA::Any& any, ::CORBA::InterfaceAttrExtension::ExtFullInterfaceDescription& value) { return any._extract<::CORBA::InterfaceAttrExtension::ExtFullInterfaceDescription>(value); }

        virtual ~InterfaceAttrExtension() = default;

        virtual ::CORBA::InterfaceAttrExtension::ExtFullInterfaceDescription describe_ext_interface() = 0;

        virtual ::omg::types::ref_type<::CORBA::ExtAttributeDef> create_ext_attribute(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, ::omg::types::ref_type<::CORBA::IDLType> type, ::CORBA::AttributeMode mode, const ::CORBA::ExceptionDefSeq& get_exceptions, const ::CORBA::ExceptionDefSeq& set_exceptions) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/InterfaceAttrExtension:1.0" || ::CORBA::Object::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/InterfaceAttrExtension:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/InterfaceAttrExtension:1.0"; }
    };
    #if __cplusplus < 201703L
    const ::omg::types::ref_type<::CORBA::TypeCode> CORBA::InterfaceAttrExtension::_tc_ExtFullInterfaceDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/InterfaceAttrExtension/ExtFullInterfaceDescription:1.0", "ExtFullInterfaceDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"operations", ::CORBA::_tc_OpDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"attributes", ::CORBA::_tc_ExtAttrDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"base_interfaces", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}});
    #endif

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_InterfaceAttrExtension = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/InterfaceAttrExtension:1.0", "InterfaceAttrExtension");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::InterfaceAttrExtension> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_InterfaceAttrExtension); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::InterfaceAttrExtension>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::InterfaceAttrExtension>(base); return !base || value; }

    class ExtInterfaceDef : public virtual ::CORBA::InterfaceDef, public virtual ::CORBA::InterfaceAttrExtension {
        public:

        virtual ~ExtInterfaceDef() = default;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ExtInterfaceDef:1.0" || ::CORBA::InterfaceDef::_is_a(logical_type_id) || ::CORBA::InterfaceAttrExtension::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ExtInterfaceDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ExtInterfaceDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtInterfaceDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ExtInterfaceDef:1.0", "ExtInterfaceDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ExtInterfaceDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ExtInterfaceDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ExtInterfaceDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ExtInterfaceDef>(base); return !base || value; }

    using ValueMemberSeq = ::omg::types::sequence<::CORBA::ValueMember>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ValueMemberSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ValueMemberSeq:1.0", "ValueMemberSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_recursive_tc("IDL:omg.org/CORBA/ValueMember:1.0")));

    class ValueDef : public virtual ::CORBA::Container, public virtual ::CORBA::Contained, public virtual ::CORBA::IDLType {
        public:

        struct FullValueDescription {
            ::CORBA::Identifier name;
            ::CORBA::RepositoryId id;
            bool is_abstract {};
            bool is_custom {};
            ::CORBA::RepositoryId defined_in;
            ::CORBA::VersionSpec version;
            ::CORBA::OpDescriptionSeq operations;
            ::CORBA::AttrDescriptionSeq attributes;
            ::CORBA::ValueMemberSeq members;
            ::CORBA::InitializerSeq initializers;
            ::CORBA::RepositoryIdSeq supported_interfaces;
            ::CORBA::RepositoryIdSeq abstract_base_values;
            bool is_truncatable {};
            ::CORBA::RepositoryId base_value;
            ::omg::types::ref_type<::CORBA::TypeCode> type;
        };
        friend bool operator==(const FullValueDescription& a, const FullValueDescription& b) {
            return a.name == b.name
                && a.id == b.id
                && a.is_abstract == b.is_abstract
                && a.is_custom == b.is_custom
                && a.defined_in == b.defined_in
                && a.version == b.version
                && a.operations == b.operations
                && a.attributes == b.attributes
                && a.members == b.members
                && a.initializers == b.initializers
                && a.supported_interfaces == b.supported_interfaces
                && a.abstract_base_values == b.abstract_base_values
                && a.is_truncatable == b.is_truncatable
                && a.base_value == b.base_value
                && a.type == b.type;
        }
        friend bool operator!=(const FullValueDescription& a, const FullValueDescription& b) {
            return !(a == b);
        }
        friend void swap(FullValueDescription& a, FullValueDescription& b) {
            using std::swap;
            swap(a.name, b.name);
            swap(a.id, b.id);
            swap(a.is_abstract, b.is_abstract);
            swap(a.is_custom, b.is_custom);
            swap(a.defined_in, b.defined_in);
            swap(a.version, b.version);
            swap(a.operations, b.operations);
            swap(a.attributes, b.attributes);
            swap(a.members, b.members);
            swap(a.initializers, b.initializers);
            swap(a.supported_interfaces, b.supported_interfaces);
            swap(a.abstract_base_values, b.abstract_base_values);
            swap(a.is_truncatable, b.is_truncatable);
            swap(a.base_value, b.base_value);
            swap(a.type, b.type);
        }
        #if __cplusplus >= 201703L
        static inline const ::omg::types::ref_type<::CORBA::TypeCode> _tc_FullValueDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ValueDef/FullValueDescription:1.0", "FullValueDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_abstract", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_custom", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"operations", ::CORBA::_tc_OpDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"attributes", ::CORBA::_tc_AttrDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"members", ::CORBA::_tc_ValueMemberSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"initializers", ::CORBA::_tc_InitializerSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"supported_interfaces", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"abstract_base_values", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_truncatable", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"base_value", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}});
        #else
        static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_FullValueDescription;
        #endif
        friend void operator<<=(::CORBA::Any& any, const ::CORBA::ValueDef::FullValueDescription& value) { any._insert<::CORBA::ValueDef::FullValueDescription>(value, _tc_FullValueDescription); }
        friend void operator<<=(::CORBA::Any& any, ::CORBA::ValueDef::FullValueDescription&& value) { any._insert<::CORBA::ValueDef::FullValueDescription>(std::move(value), _tc_FullValueDescription); }
        friend bool operator>>=(const ::CORBA::Any& any, ::CORBA::ValueDef::FullValueDescription& value) { return any._extract<::CORBA::ValueDef::FullValueDescription>(value); }

        virtual ~ValueDef() = default;

        virtual void supported_interfaces(const ::CORBA::InterfaceDefSeq& value) = 0;
        virtual ::CORBA::InterfaceDefSeq supported_interfaces() const = 0;

        virtual void initializers(const ::CORBA::InitializerSeq& value) = 0;
        virtual ::CORBA::InitializerSeq initializers() const = 0;

        virtual void base_value(::omg::types::ref_type<::CORBA::ValueDef> value) = 0;
        virtual ::omg::types::ref_type<::CORBA::ValueDef> base_value() const = 0;

        virtual void abstract_base_values(const ::CORBA::ValueDefSeq& value) = 0;
        virtual ::CORBA::ValueDefSeq abstract_base_values() const = 0;

        virtual void is_abstract(bool value) = 0;
        virtual bool is_abstract() const = 0;

        virtual void is_custom(bool value) = 0;
        virtual bool is_custom() const = 0;

        virtual void is_truncatable(bool value) = 0;
        virtual bool is_truncatable() const = 0;

        virtual bool is_a(const ::CORBA::RepositoryId& id) = 0;

        virtual ::CORBA::ValueDef::FullValueDescription describe_value() = 0;

        virtual ::omg::types::ref_type<::CORBA::ValueMemberDef> create_value_member(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, ::omg::types::ref_type<::CORBA::IDLType> type, ::CORBA::Visibility access) = 0;

        virtual ::omg::types::ref_type<::CORBA::AttributeDef> create_attribute(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, ::omg::types::ref_type<::CORBA::IDLType> type, ::CORBA::AttributeMode mode) = 0;

        virtual ::omg::types::ref_type<::CORBA::OperationDef> create_operation(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, ::omg::types::ref_type<::CORBA::IDLType> result, ::CORBA::OperationMode mode, const ::CORBA::ParDescriptionSeq& params, const ::CORBA::ExceptionDefSeq& exceptions, const ::CORBA::ContextIdSeq& contexts) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ValueDef:1.0" || ::CORBA::Container::_is_a(logical_type_id) || ::CORBA::Contained::_is_a(logical_type_id) || ::CORBA::IDLType::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ValueDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ValueDef:1.0"; }
    };
    #if __cplusplus < 201703L
    const ::omg::types::ref_type<::CORBA::TypeCode> CORBA::ValueDef::_tc_FullValueDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ValueDef/FullValueDescription:1.0", "FullValueDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_abstract", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_custom", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"operations", ::CORBA::_tc_OpDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"attributes", ::CORBA::_tc_AttrDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"members", ::CORBA::_tc_ValueMemberSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"initializers", ::CORBA::_tc_InitializerSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"supported_interfaces", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"abstract_base_values", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_truncatable", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"base_value", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}});
    #endif

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ValueDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ValueDef:1.0", "ValueDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ValueDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ValueDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ValueDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ValueDef>(base); return !base || value; }

    class ExtValueDef : public virtual ::CORBA::ValueDef {
        public:

        struct ExtFullValueDescription {
            ::CORBA::Identifier name;
            ::CORBA::RepositoryId id;
            bool is_abstract {};
            bool is_custom {};
            ::CORBA::RepositoryId defined_in;
            ::CORBA::VersionSpec version;
            ::CORBA::OpDescriptionSeq operations;
            ::CORBA::ExtAttrDescriptionSeq attributes;
            ::CORBA::ValueMemberSeq members;
            ::CORBA::ExtInitializerSeq initializers;
            ::CORBA::RepositoryIdSeq supported_interfaces;
            ::CORBA::RepositoryIdSeq abstract_base_values;
            bool is_truncatable {};
            ::CORBA::RepositoryId base_value;
            ::omg::types::ref_type<::CORBA::TypeCode> type;
        };
        friend bool operator==(const ExtFullValueDescription& a, const ExtFullValueDescription& b) {
            return a.name == b.name
                && a.id == b.id
                && a.is_abstract == b.is_abstract
                && a.is_custom == b.is_custom
                && a.defined_in == b.defined_in
                && a.version == b.version
                && a.operations == b.operations
                && a.attributes == b.attributes
                && a.members == b.members
                && a.initializers == b.initializers
                && a.supported_interfaces == b.supported_interfaces
                && a.abstract_base_values == b.abstract_base_values
                && a.is_truncatable == b.is_truncatable
                && a.base_value == b.base_value
                && a.type == b.type;
        }
        friend bool operator!=(const ExtFullValueDescription& a, const ExtFullValueDescription& b) {
            return !(a == b);
        }
        friend void swap(ExtFullValueDescription& a, ExtFullValueDescription& b) {
            using std::swap;
            swap(a.name, b.name);
            swap(a.id, b.id);
            swap(a.is_abstract, b.is_abstract);
            swap(a.is_custom, b.is_custom);
            swap(a.defined_in, b.defined_in);
            swap(a.version, b.version);
            swap(a.operations, b.operations);
            swap(a.attributes, b.attributes);
            swap(a.members, b.members);
            swap(a.initializers, b.initializers);
            swap(a.supported_interfaces, b.supported_interfaces);
            swap(a.abstract_base_values, b.abstract_base_values);
            swap(a.is_truncatable, b.is_truncatable);
            swap(a.base_value, b.base_value);
            swap(a.type, b.type);
        }
        #if __cplusplus >= 201703L
        static inline const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtFullValueDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ExtValueDef/ExtFullValueDescription:1.0", "ExtFullValueDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_abstract", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_custom", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"operations", ::CORBA::_tc_OpDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"attributes", ::CORBA::_tc_ExtAttrDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"members", ::CORBA::_tc_ValueMemberSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"initializers", ::CORBA::_tc_ExtInitializerSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"supported_interfaces", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"abstract_base_values", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_truncatable", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"base_value", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}});
        #else
        static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtFullValueDescription;
        #endif
        friend void operator<<=(::CORBA::Any& any, const ::CORBA::ExtValueDef::ExtFullValueDescription& value) { any._insert<::CORBA::ExtValueDef::ExtFullValueDescription>(value, _tc_ExtFullValueDescription); }
        friend void operator<<=(::CORBA::Any& any, ::CORBA::ExtValueDef::ExtFullValueDescription&& value) { any._insert<::CORBA::ExtValueDef::ExtFullValueDescription>(std::move(value), _tc_ExtFullValueDescription); }
        friend bool operator>>=(const ::CORBA::Any& any, ::CORBA::ExtValueDef::ExtFullValueDescription& value) { return any._extract<::CORBA::ExtValueDef::ExtFullValueDescription>(value); }

        virtual ~ExtValueDef() = default;

        virtual void ext_initializers(const ::CORBA::ExtInitializerSeq& value) = 0;
        virtual ::CORBA::ExtInitializerSeq ext_initializers() const = 0;

        virtual ::CORBA::ExtValueDef::ExtFullValueDescription describe_ext_value() = 0;

        virtual ::omg::types::ref_type<::CORBA::ExtAttributeDef> create_ext_attribute(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::VersionSpec& version, ::omg::types::ref_type<::CORBA::IDLType> type, ::CORBA::AttributeMode mode, const ::CORBA::ExceptionDefSeq& get_exceptions, const ::CORBA::ExceptionDefSeq& set_exceptions) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ExtValueDef:1.0" || ::CORBA::ValueDef::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ExtValueDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ExtValueDef:1.0"; }
    };
    #if __cplusplus < 201703L
    const ::omg::types::ref_type<::CORBA::TypeCode> CORBA::ExtValueDef::_tc_ExtFullValueDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ExtValueDef/ExtFullValueDescription:1.0", "ExtFullValueDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_abstract", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_custom", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"operations", ::CORBA::_tc_OpDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"attributes", ::CORBA::_tc_ExtAttrDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"members", ::CORBA::_tc_ValueMemberSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"initializers", ::CORBA::_tc_ExtInitializerSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"supported_interfaces", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"abstract_base_values", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_truncatable", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"base_value", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}});
    #endif

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtValueDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ExtValueDef:1.0", "ExtValueDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ExtValueDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ExtValueDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ExtValueDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ExtValueDef>(base); return !base || value; }

    class ExtAbstractInterfaceDef : public virtual ::CORBA::AbstractInterfaceDef, public virtual ::CORBA::InterfaceAttrExtension {
        public:

        virtual ~ExtAbstractInterfaceDef() = default;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ExtAbstractInterfaceDef:1.0" || ::CORBA::AbstractInterfaceDef::_is_a(logical_type_id) || ::CORBA::InterfaceAttrExtension::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ExtAbstractInterfaceDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ExtAbstractInterfaceDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtAbstractInterfaceDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ExtAbstractInterfaceDef:1.0", "ExtAbstractInterfaceDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ExtAbstractInterfaceDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ExtAbstractInterfaceDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ExtAbstractInterfaceDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ExtAbstractInterfaceDef>(base); return !base || value; }

    class LocalInterfaceDef : public virtual ::CORBA::InterfaceDef {
        public:

        virtual ~LocalInterfaceDef() = default;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/LocalInterfaceDef:1.0" || ::CORBA::InterfaceDef::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/LocalInterfaceDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/LocalInterfaceDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_LocalInterfaceDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/LocalInterfaceDef:1.0", "LocalInterfaceDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::LocalInterfaceDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_LocalInterfaceDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::LocalInterfaceDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::LocalInterfaceDef>(base); return !base || value; }

    class ExtLocalInterfaceDef : public virtual ::CORBA::LocalInterfaceDef, public virtual ::CORBA::InterfaceAttrExtension {
        public:

        virtual ~ExtLocalInterfaceDef() = default;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ExtLocalInterfaceDef:1.0" || ::CORBA::LocalInterfaceDef::_is_a(logical_type_id) || ::CORBA::InterfaceAttrExtension::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ExtLocalInterfaceDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ExtLocalInterfaceDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtLocalInterfaceDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ExtLocalInterfaceDef:1.0", "ExtLocalInterfaceDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ExtLocalInterfaceDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ExtLocalInterfaceDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ExtLocalInterfaceDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ExtLocalInterfaceDef>(base); return !base || value; }

    class FixedDef : public virtual ::CORBA::IDLType {
        public:

        virtual ~FixedDef() = default;

        virtual void digits(std::uint16_t value) = 0;
        virtual std::uint16_t digits() const = 0;

        virtual void scale(std::int16_t value) = 0;
        virtual std::int16_t scale() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/FixedDef:1.0" || ::CORBA::IDLType::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/FixedDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/FixedDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_FixedDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/FixedDef:1.0", "FixedDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::FixedDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_FixedDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::FixedDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::FixedDef>(base); return !base || value; }

    class ModuleDef : public virtual ::CORBA::Container, public virtual ::CORBA::Contained {
        public:

        virtual ~ModuleDef() = default;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ModuleDef:1.0" || ::CORBA::Container::_is_a(logical_type_id) || ::CORBA::Contained::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ModuleDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ModuleDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ModuleDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ModuleDef:1.0", "ModuleDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ModuleDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ModuleDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ModuleDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ModuleDef>(base); return !base || value; }

    class NativeDef : public virtual ::CORBA::TypedefDef {
        public:

        virtual ~NativeDef() = default;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/NativeDef:1.0" || ::CORBA::TypedefDef::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/NativeDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/NativeDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_NativeDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/NativeDef:1.0", "NativeDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::NativeDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_NativeDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::NativeDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::NativeDef>(base); return !base || value; }

    class OperationDef : public virtual ::CORBA::Contained {
        public:

        virtual ~OperationDef() = default;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> result() const = 0;

        virtual void result_def(::omg::types::ref_type<::CORBA::IDLType> value) = 0;
        virtual ::omg::types::ref_type<::CORBA::IDLType> result_def() const = 0;

        virtual void params(const ::CORBA::ParDescriptionSeq& value) = 0;
        virtual ::CORBA::ParDescriptionSeq params() const = 0;

        virtual void mode(::CORBA::OperationMode value) = 0;
        virtual ::CORBA::OperationMode mode() const = 0;

        virtual void contexts(const ::CORBA::ContextIdSeq& value) = 0;
        virtual ::CORBA::ContextIdSeq contexts() const = 0;

        virtual void exceptions(const ::CORBA::ExceptionDefSeq& value) = 0;
        virtual ::CORBA::ExceptionDefSeq exceptions() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/OperationDef:1.0" || ::CORBA::Contained::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/OperationDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/OperationDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_OperationDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/OperationDef:1.0", "OperationDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::OperationDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_OperationDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::OperationDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::OperationDef>(base); return !base || value; }

    enum class PrimitiveKind {
        pk_null = 0,
        pk_void = 1,
        pk_short = 2,
        pk_long = 3,
        pk_ushort = 4,
        pk_ulong = 5,
        pk_float = 6,
        pk_double = 7,
        pk_boolean = 8,
        pk_char = 9,
        pk_octet = 10,
        pk_any = 11,
        pk_TypeCode = 12,
        pk_Principal = 13,
        pk_string = 14,
        pk_objref = 15,
        pk_longlong = 16,
        pk_ulonglong = 17,
        pk_longdouble = 18,
        pk_wchar = 19,
        pk_wstring = 20,
        pk_value_base = 21
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_PrimitiveKind = ::CORBA::TypeCode::create_enum_tc("IDL:omg.org/CORBA/PrimitiveKind:1.0", "PrimitiveKind", {"pk_null", "pk_void", "pk_short", "pk_long", "pk_ushort", "pk_ulong", "pk_float", "pk_double", "pk_boolean", "pk_char", "pk_octet", "pk_any", "pk_TypeCode", "pk_Principal", "pk_string", "pk_objref", "pk_longlong", "pk_ulonglong", "pk_longdouble", "pk_wchar", "pk_wstring", "pk_value_base"});
    inline void operator<<=(::CORBA::Any& any, ::CORBA::PrimitiveKind value) { any._insert<::CORBA::PrimitiveKind>(value, _tc_PrimitiveKind); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::PrimitiveKind& value) { return any._extract<::CORBA::PrimitiveKind>(value); }

    class PrimitiveDef : public virtual ::CORBA::IDLType {
        public:

        virtual ~PrimitiveDef() = default;

        virtual ::CORBA::PrimitiveKind kind() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/PrimitiveDef:1.0" || ::CORBA::IDLType::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/PrimitiveDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/PrimitiveDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_PrimitiveDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/PrimitiveDef:1.0", "PrimitiveDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::PrimitiveDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_PrimitiveDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::PrimitiveDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::PrimitiveDef>(base); return !base || value; }

    class Repository : public virtual ::CORBA::Container {
        public:

        virtual ~Repository() = default;

        virtual ::omg::types::ref_type<::CORBA::Contained> lookup_id(const ::CORBA::RepositoryId& search_id) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> get_canonical_typecode(::omg::types::ref_type<::CORBA::TypeCode> tc) = 0;

        virtual ::omg::types::ref_type<::CORBA::PrimitiveDef> get_primitive(::CORBA::PrimitiveKind kind) = 0;

        virtual ::omg::types::ref_type<::CORBA::StringDef> create_string(std::uint32_t bound) = 0;

        virtual ::omg::types::ref_type<::CORBA::WstringDef> create_wstring(std::uint32_t bound) = 0;

        virtual ::omg::types::ref_type<::CORBA::SequenceDef> create_sequence(std::uint32_t bound, ::omg::types::ref_type<::CORBA::IDLType> element_type) = 0;

        virtual ::omg::types::ref_type<::CORBA::ArrayDef> create_array(std::uint32_t length, ::omg::types::ref_type<::CORBA::IDLType> element_type) = 0;

        virtual ::omg::types::ref_type<::CORBA::FixedDef> create_fixed(std::uint16_t digits, std::int16_t scale) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/Repository:1.0" || ::CORBA::Container::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/Repository:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/Repository:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Repository = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/Repository:1.0", "Repository");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Repository> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_Repository); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Repository>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::Repository>(base); return !base || value; }

    class SequenceDef : public virtual ::CORBA::IDLType {
        public:

        virtual ~SequenceDef() = default;

        virtual void bound(std::uint32_t value) = 0;
        virtual std::uint32_t bound() const = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> element_type() const = 0;

        virtual void element_type_def(::omg::types::ref_type<::CORBA::IDLType> value) = 0;
        virtual ::omg::types::ref_type<::CORBA::IDLType> element_type_def() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/SequenceDef:1.0" || ::CORBA::IDLType::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/SequenceDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/SequenceDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_SequenceDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/SequenceDef:1.0", "SequenceDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::SequenceDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_SequenceDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::SequenceDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::SequenceDef>(base); return !base || value; }

    class StringDef : public virtual ::CORBA::IDLType {
        public:

        virtual ~StringDef() = default;

        virtual void bound(std::uint32_t value) = 0;
        virtual std::uint32_t bound() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/StringDef:1.0" || ::CORBA::IDLType::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/StringDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/StringDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_StringDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/StringDef:1.0", "StringDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::StringDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_StringDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::StringDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::StringDef>(base); return !base || value; }

    class StructDef : public virtual ::CORBA::TypedefDef, public virtual ::CORBA::Container {
        public:

        virtual ~StructDef() = default;

        virtual void members(const ::CORBA::StructMemberSeq& value) = 0;
        virtual ::CORBA::StructMemberSeq members() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/StructDef:1.0" || ::CORBA::TypedefDef::_is_a(logical_type_id) || ::CORBA::Container::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/StructDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/StructDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_StructDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/StructDef:1.0", "StructDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::StructDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_StructDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::StructDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::StructDef>(base); return !base || value; }

    class UnionDef : public virtual ::CORBA::TypedefDef, public virtual ::CORBA::Container {
        public:

        virtual ~UnionDef() = default;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> discriminator_type() const = 0;

        virtual void discriminator_type_def(::omg::types::ref_type<::CORBA::IDLType> value) = 0;
        virtual ::omg::types::ref_type<::CORBA::IDLType> discriminator_type_def() const = 0;

        virtual void members(const ::CORBA::UnionMemberSeq& value) = 0;
        virtual ::CORBA::UnionMemberSeq members() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/UnionDef:1.0" || ::CORBA::TypedefDef::_is_a(logical_type_id) || ::CORBA::Container::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/UnionDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/UnionDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_UnionDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/UnionDef:1.0", "UnionDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::UnionDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_UnionDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::UnionDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::UnionDef>(base); return !base || value; }

    class ValueBoxDef : public virtual ::CORBA::TypedefDef {
        public:

        virtual ~ValueBoxDef() = default;

        virtual void original_type_def(::omg::types::ref_type<::CORBA::IDLType> value) = 0;
        virtual ::omg::types::ref_type<::CORBA::IDLType> original_type_def() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ValueBoxDef:1.0" || ::CORBA::TypedefDef::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ValueBoxDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ValueBoxDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ValueBoxDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ValueBoxDef:1.0", "ValueBoxDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ValueBoxDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ValueBoxDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ValueBoxDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ValueBoxDef>(base); return !base || value; }

    class ValueMemberDef : public virtual ::CORBA::Contained {
        public:

        virtual ~ValueMemberDef() = default;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> type() const = 0;

        virtual void type_def(::omg::types::ref_type<::CORBA::IDLType> value) = 0;
        virtual ::omg::types::ref_type<::CORBA::IDLType> type_def() const = 0;

        virtual void access(::CORBA::Visibility value) = 0;
        virtual ::CORBA::Visibility access() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ValueMemberDef:1.0" || ::CORBA::Contained::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ValueMemberDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ValueMemberDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ValueMemberDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ValueMemberDef:1.0", "ValueMemberDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ValueMemberDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ValueMemberDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ValueMemberDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ValueMemberDef>(base); return !base || value; }

    class WstringDef : public virtual ::CORBA::IDLType {
        public:

        virtual ~WstringDef() = default;

        virtual void bound(std::uint32_t value) = 0;
        virtual std::uint32_t bound() const = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/WstringDef:1.0" || ::CORBA::IDLType::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/WstringDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/WstringDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_WstringDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/WstringDef:1.0", "WstringDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::WstringDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_WstringDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::WstringDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::WstringDef>(base); return !base || value; }

    constexpr std::uint32_t OMGVMCID = 1330446336U;

    enum class exception_type {
        NO_EXCEPTION = 0,
        USER_EXCEPTION = 1,
        SYSTEM_EXCEPTION = 2
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_exception_type = ::CORBA::TypeCode::create_enum_tc("IDL:omg.org/CORBA/exception_type:1.0", "exception_type", {"NO_EXCEPTION", "USER_EXCEPTION", "SYSTEM_EXCEPTION"});
    inline void operator<<=(::CORBA::Any& any, ::CORBA::exception_type value) { any._insert<::CORBA::exception_type>(value, _tc_exception_type); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::exception_type& value) { return any._extract<::CORBA::exception_type>(value); }

    using PolicyList = ::omg::types::sequence<::omg::types::ref_type<::CORBA::Policy>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_PolicyList = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/PolicyList:1.0", "PolicyList", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/Policy:1.0", "Policy")));

    using PolicyTypeSeq = ::omg::types::sequence<::CORBA::PolicyType>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_PolicyTypeSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/PolicyTypeSeq:1.0", "PolicyTypeSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_PolicyType));

    using PolicyErrorCode = std::int16_t;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_PolicyErrorCode = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/PolicyErrorCode:1.0", "PolicyErrorCode", ::CORBA::_tc_short);

    class PolicyError : public ::CORBA::UserException {
        public:
        PolicyError() : reason_(), what_("PolicyError") {}
        PolicyError(const PolicyError&) = default;
        PolicyError(PolicyError&&) = default;
        explicit PolicyError(::CORBA::PolicyErrorCode reason, const char* what) : reason_(reason), what_(what ? what : "") {}
        ~PolicyError() noexcept override = default;
        PolicyError& operator=(const PolicyError&) = default;
        PolicyError& operator=(PolicyError&&) = default;

        void reason(::CORBA::PolicyErrorCode value) { reason_ = value; }
        ::CORBA::PolicyErrorCode reason() const { return reason_; }

        void raise() const override { throw *this; }
        const char* _name() const override { return "PolicyError"; }
        const char* _rep_id() const override { return "IDL:omg.org/CORBA/PolicyError:1.0"; }
        void _insert_into(::CORBA::Any& any) const override;

        const char* what() const noexcept override { return what_.c_str(); }

        private:
        ::CORBA::PolicyErrorCode reason_;
        std::string what_;
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_PolicyError = ::CORBA::TypeCode::create_exception_tc("IDL:omg.org/CORBA/PolicyError:1.0", "PolicyError", {::CORBA::TypeCode::Member{"reason", ::CORBA::_tc_PolicyErrorCode, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::PolicyError& value) { any._insert<::CORBA::PolicyError>(value, _tc_PolicyError); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::PolicyError&& value) { any._insert<::CORBA::PolicyError>(std::move(value), _tc_PolicyError); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::PolicyError& value) { return any._extract<::CORBA::PolicyError>(value); }
    inline void ::CORBA::PolicyError::_insert_into(::CORBA::Any& any) const { any <<= *this; }

    constexpr ::CORBA::PolicyErrorCode BAD_POLICY = 0;

    constexpr ::CORBA::PolicyErrorCode UNSUPPORTED_POLICY = 1;

    constexpr ::CORBA::PolicyErrorCode BAD_POLICY_TYPE = 2;

    constexpr ::CORBA::PolicyErrorCode BAD_POLICY_VALUE = 3;

    constexpr ::CORBA::PolicyErrorCode UNSUPPORTED_POLICY_VALUE = 4;

    enum class SetOverrideType {
        SET_OVERRIDE = 0,
        ADD_OVERRIDE = 1
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_SetOverrideType = ::CORBA::TypeCode::create_enum_tc("IDL:omg.org/CORBA/SetOverrideType:1.0", "SetOverrideType", {"SET_OVERRIDE", "ADD_OVERRIDE"});
    inline void operator<<=(::CORBA::Any& any, ::CORBA::SetOverrideType value) { any._insert<::CORBA::SetOverrideType>(value, _tc_SetOverrideType); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::SetOverrideType& value) { return any._extract<::CORBA::SetOverrideType>(value); }

    class InvalidPolicies : public ::CORBA::UserException {
        public:
        InvalidPolicies() : what_("InvalidPolicies") {}
        InvalidPolicies(const InvalidPolicies&) = default;
        InvalidPolicies(InvalidPolicies&&) = default;
        explicit InvalidPolicies(const ::omg::types::sequence<std::uint16_t>& indicies, const char* what) : indicies_(indicies), what_(what ? what : "") {}
        ~InvalidPolicies() noexcept override = default;
        InvalidPolicies& operator=(const InvalidPolicies&) = default;
        InvalidPolicies& operator=(InvalidPolicies&&) = default;

        void indicies(const ::omg::types::sequence<std::uint16_t>& value) { indicies_ = value; }
        const ::omg::types::sequence<std::uint16_t>& indicies() const { return indicies_; }
        ::omg::types::sequence<std::uint16_t>& indicies() { return indicies_; }

        void raise() const override { throw *this; }
        const char* _name() const override { return "InvalidPolicies"; }
        const char* _rep_id() const override { return "IDL:omg.org/CORBA/InvalidPolicies:1.0"; }
        void _insert_into(::CORBA::Any& any) const override;

        const char* what() const noexcept override { return what_.c_str(); }

        private:
        ::omg::types::sequence<std::uint16_t> indicies_;
        std::string what_;
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_InvalidPolicies = ::CORBA::TypeCode::create_exception_tc("IDL:omg.org/CORBA/InvalidPolicies:1.0", "InvalidPolicies", {::CORBA::TypeCode::Member{"indicies", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_ushort), nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::InvalidPolicies& value) { any._insert<::CORBA::InvalidPolicies>(value, _tc_InvalidPolicies); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::InvalidPolicies&& value) { any._insert<::CORBA::InvalidPolicies>(std::move(value), _tc_InvalidPolicies); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::InvalidPolicies& value) { return any._extract<::CORBA::InvalidPolicies>(value); }
    inline void ::CORBA::InvalidPolicies::_insert_into(::CORBA::Any& any) const { any <<= *this; }

    class PolicyManager : public virtual ::CORBA::Object {
        public:

        virtual ~PolicyManager() = default;

        virtual ::CORBA::PolicyList get_policy_overrides(const ::CORBA::PolicyTypeSeq& ts) = 0;

        virtual void set_policy_overrides(const ::CORBA::PolicyList& policies, ::CORBA::SetOverrideType set_add) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/PolicyManager:1.0" || ::CORBA::Object::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/PolicyManager:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/PolicyManager:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_PolicyManager = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/PolicyManager:1.0", "PolicyManager");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::PolicyManager> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_PolicyManager); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::PolicyManager>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::PolicyManager>(base); return !base || value; }

    class PolicyCurrent : public virtual ::CORBA::PolicyManager, public virtual ::CORBA::Current {
        public:

        virtual ~PolicyCurrent() = default;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/PolicyCurrent:1.0" || ::CORBA::PolicyManager::_is_a(logical_type_id) || ::CORBA::Current::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/PolicyCurrent:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/PolicyCurrent:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_PolicyCurrent = ::CORBA::TypeCode::create_local_interface_tc("IDL:omg.org/CORBA/PolicyCurrent:1.0", "PolicyCurrent");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::PolicyCurrent> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_PolicyCurrent); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::PolicyCurrent>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::PolicyCurrent>(base); return !base || value; }

    constexpr ::CORBA::PolicyType SecConstruction = 11U;

    using DomainManagersList = ::omg::types::sequence<::omg::types::ref_type<::CORBA::DomainManager>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_DomainManagersList = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/DomainManagersList:1.0", "DomainManagersList", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/DomainManager:1.0", "DomainManager")));

    class Pollable : public virtual ::CORBA::ValueBase {
        public:
        virtual bool is_ready(std::uint32_t timeout) = 0;
        virtual ::omg::types::ref_type<::CORBA::PollableSet> create_pollable_set() = 0;

        protected:
        Pollable() = default;
        Pollable(const Pollable&) = default;
        Pollable(Pollable&&) = default;
        ~Pollable() override = default;

        private:
        Pollable& operator=(const Pollable&) = delete;
        Pollable& operator=(Pollable&&) = delete;
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Pollable = ::CORBA::TypeCode::create_value_tc("IDL:omg.org/CORBA/Pollable:1.0", "Pollable", ::CORBA::VM_ABSTRACT, nullptr, {});
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Pollable> value) { any._insert<::omg::types::ref_type<::CORBA::ValueBase>>(std::move(value), _tc_Pollable); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Pollable>& value) { ::omg::types::ref_type<::CORBA::ValueBase> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::Pollable>(base); return !base || value; }

    class DIIPollable : public virtual ::CORBA::Pollable {
        public:

        protected:
        DIIPollable() = default;
        DIIPollable(const DIIPollable&) = default;
        DIIPollable(DIIPollable&&) = default;
        ~DIIPollable() override = default;

        private:
        DIIPollable& operator=(const DIIPollable&) = delete;
        DIIPollable& operator=(DIIPollable&&) = delete;
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_DIIPollable = ::CORBA::TypeCode::create_value_tc("IDL:omg.org/CORBA/DIIPollable:1.0", "DIIPollable", ::CORBA::VM_ABSTRACT, ::CORBA::_tc_Pollable, {});
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::DIIPollable> value) { any._insert<::omg::types::ref_type<::CORBA::ValueBase>>(std::move(value), _tc_DIIPollable); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::DIIPollable>& value) { ::omg::types::ref_type<::CORBA::ValueBase> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::DIIPollable>(base); return !base || value; }

    using LocalInterfaceDefSeq = ::omg::types::sequence<::omg::types::ref_type<::CORBA::LocalInterfaceDef>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_LocalInterfaceDefSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/LocalInterfaceDefSeq:1.0", "LocalInterfaceDefSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/LocalInterfaceDef:1.0", "LocalInterfaceDef")));

    using ExtInterfaceDefSeq = ::omg::types::sequence<::omg::types::ref_type<::CORBA::ExtInterfaceDef>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtInterfaceDefSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ExtInterfaceDefSeq:1.0", "ExtInterfaceDefSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ExtInterfaceDef:1.0", "ExtInterfaceDef")));

    using ExtValueDefSeq = ::omg::types::sequence<::omg::types::ref_type<::CORBA::ExtValueDef>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtValueDefSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ExtValueDefSeq:1.0", "ExtValueDefSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ExtValueDef:1.0", "ExtValueDef")));

    using ExtAbstractInterfaceDefSeq = ::omg::types::sequence<::omg::types::ref_type<::CORBA::ExtAbstractInterfaceDef>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtAbstractInterfaceDefSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ExtAbstractInterfaceDefSeq:1.0", "ExtAbstractInterfaceDefSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ExtAbstractInterfaceDef:1.0", "ExtAbstractInterfaceDef")));

    using ExtLocalInterfaceDefSeq = ::omg::types::sequence<::omg::types::ref_type<::CORBA::ExtLocalInterfaceDef>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtLocalInterfaceDefSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ExtLocalInterfaceDefSeq:1.0", "ExtLocalInterfaceDefSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ExtLocalInterfaceDef:1.0", "ExtLocalInterfaceDef")));

    struct StructMember {
        ::CORBA::Identifier name;
        ::omg::types::ref_type<::CORBA::TypeCode> type;
        ::omg::types::ref_type<::CORBA::IDLType> type_def;
    };
    inline bool operator==(const StructMember& a, const StructMember& b) {
        return a.name == b.name
            && a.type == b.type
            && a.type_def == b.type_def;
    }
    inline bool operator!=(const StructMember& a, const StructMember& b) {
        return !(a == b);
    }
    inline void swap(StructMember& a, StructMember& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.type, b.type);
        swap(a.type_def, b.type_def);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_StructMember = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/StructMember:1.0", "StructMember", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type_def", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/IDLType:1.0", "IDLType"), nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::StructMember& value) { any._insert<::CORBA::StructMember>(value, _tc_StructMember); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::StructMember&& value) { any._insert<::CORBA::StructMember>(std::move(value), _tc_StructMember); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::StructMember& value) { return any._extract<::CORBA::StructMember>(value); }

    struct Initializer {
        ::CORBA::StructMemberSeq members;
        ::CORBA::Identifier name;
    };
    inline bool operator==(const Initializer& a, const Initializer& b) {
        return a.members == b.members
            && a.name == b.name;
    }
    inline bool operator!=(const Initializer& a, const Initializer& b) {
        return !(a == b);
    }
    inline void swap(Initializer& a, Initializer& b) {
        using std::swap;
        swap(a.members, b.members);
        swap(a.name, b.name);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Initializer = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/Initializer:1.0", "Initializer", {::CORBA::TypeCode::Member{"members", ::CORBA::_tc_StructMemberSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::Initializer& value) { any._insert<::CORBA::Initializer>(value, _tc_Initializer); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::Initializer&& value) { any._insert<::CORBA::Initializer>(std::move(value), _tc_Initializer); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::Initializer& value) { return any._extract<::CORBA::Initializer>(value); }

    struct UnionMember {
        ::CORBA::Identifier name;
        ::CORBA::Any label;
        ::omg::types::ref_type<::CORBA::TypeCode> type;
        ::omg::types::ref_type<::CORBA::IDLType> type_def;
    };
    inline bool operator==(const UnionMember& a, const UnionMember& b) {
        return a.name == b.name
            && a.label == b.label
            && a.type == b.type
            && a.type_def == b.type_def;
    }
    inline bool operator!=(const UnionMember& a, const UnionMember& b) {
        return !(a == b);
    }
    inline void swap(UnionMember& a, UnionMember& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.label, b.label);
        swap(a.type, b.type);
        swap(a.type_def, b.type_def);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_UnionMember = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/UnionMember:1.0", "UnionMember", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"label", ::CORBA::_tc_any, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type_def", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/IDLType:1.0", "IDLType"), nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::UnionMember& value) { any._insert<::CORBA::UnionMember>(value, _tc_UnionMember); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::UnionMember&& value) { any._insert<::CORBA::UnionMember>(std::move(value), _tc_UnionMember); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::UnionMember& value) { return any._extract<::CORBA::UnionMember>(value); }

    struct ExceptionDescription {
        ::CORBA::Identifier name;
        ::CORBA::RepositoryId id;
        ::CORBA::RepositoryId defined_in;
        ::CORBA::VersionSpec version;
        ::omg::types::ref_type<::CORBA::TypeCode> type;
    };
    inline bool operator==(const ExceptionDescription& a, const ExceptionDescription& b) {
        return a.name == b.name
            && a.id == b.id
            && a.defined_in == b.defined_in
            && a.version == b.version
            && a.type == b.type;
    }
    inline bool operator!=(const ExceptionDescription& a, const ExceptionDescription& b) {
        return !(a == b);
    }
    inline void swap(ExceptionDescription& a, ExceptionDescription& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.id, b.id);
        swap(a.defined_in, b.defined_in);
        swap(a.version, b.version);
        swap(a.type, b.type);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExceptionDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ExceptionDescription:1.0", "ExceptionDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::ExceptionDescription& value) { any._insert<::CORBA::ExceptionDescription>(value, _tc_ExceptionDescription); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::ExceptionDescription&& value) { any._insert<::CORBA::ExceptionDescription>(std::move(value), _tc_ExceptionDescription); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::ExceptionDescription& value) { return any._extract<::CORBA::ExceptionDescription>(value); }

    using ExcDescriptionSeq = ::omg::types::sequence<::CORBA::ExceptionDescription>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExcDescriptionSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ExcDescriptionSeq:1.0", "ExcDescriptionSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_ExceptionDescription));

    struct ExtInitializer {
        ::CORBA::StructMemberSeq members;
        ::CORBA::ExcDescriptionSeq exceptions;
        ::CORBA::Identifier name;
    };
    inline bool operator==(const ExtInitializer& a, const ExtInitializer& b) {
        return a.members == b.members
            && a.exceptions == b.exceptions
            && a.name == b.name;
    }
    inline bool operator!=(const ExtInitializer& a, const ExtInitializer& b) {
        return !(a == b);
    }
    inline void swap(ExtInitializer& a, ExtInitializer& b) {
        using std::swap;
        swap(a.members, b.members);
        swap(a.exceptions, b.exceptions);
        swap(a.name, b.name);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtInitializer = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ExtInitializer:1.0", "ExtInitializer", {::CORBA::TypeCode::Member{"members", ::CORBA::_tc_StructMemberSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"exceptions", ::CORBA::_tc_ExcDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::ExtInitializer& value) { any._insert<::CORBA::ExtInitializer>(value, _tc_ExtInitializer); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::ExtInitializer&& value) { any._insert<::CORBA::ExtInitializer>(std::move(value), _tc_ExtInitializer); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::ExtInitializer& value) { return any._extract<::CORBA::ExtInitializer>(value); }

    struct ModuleDescription {
        ::CORBA::Identifier name;
        ::CORBA::RepositoryId id;
        ::CORBA::RepositoryId defined_in;
        ::CORBA::VersionSpec version;
    };
    inline bool operator==(const ModuleDescription& a, const ModuleDescription& b) {
        return a.name == b.name
            && a.id == b.id
            && a.defined_in == b.defined_in
            && a.version == b.version;
    }
    inline bool operator!=(const ModuleDescription& a, const ModuleDescription& b) {
        return !(a == b);
    }
    inline void swap(ModuleDescription& a, ModuleDescription& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.id, b.id);
        swap(a.defined_in, b.defined_in);
        swap(a.version, b.version);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ModuleDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ModuleDescription:1.0", "ModuleDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::ModuleDescription& value) { any._insert<::CORBA::ModuleDescription>(value, _tc_ModuleDescription); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::ModuleDescription&& value) { any._insert<::CORBA::ModuleDescription>(std::move(value), _tc_ModuleDescription); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::ModuleDescription& value) { return any._extract<::CORBA::ModuleDescription>(value); }

    struct ConstantDescription {
        ::CORBA::Identifier name;
        ::CORBA::RepositoryId id;
        ::CORBA::RepositoryId defined_in;
        ::CORBA::VersionSpec version;
        ::omg::types::ref_type<::CORBA::TypeCode> type;
        ::CORBA::Any value;
    };
    inline bool operator==(const ConstantDescription& a, const ConstantDescription& b) {
        return a.name == b.name
            && a.id == b.id
            && a.defined_in == b.defined_in
            && a.version == b.version
            && a.type == b.type
            && a.value == b.value;
    }
    inline bool operator!=(const ConstantDescription& a, const ConstantDescription& b) {
        return !(a == b);
    }
    inline void swap(ConstantDescription& a, ConstantDescription& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.id, b.id);
        swap(a.defined_in, b.defined_in);
        swap(a.version, b.version);
        swap(a.type, b.type);
        swap(a.value, b.value);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ConstantDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ConstantDescription:1.0", "ConstantDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"value", ::CORBA::_tc_any, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::ConstantDescription& value) { any._insert<::CORBA::ConstantDescription>(value, _tc_ConstantDescription); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::ConstantDescription&& value) { any._insert<::CORBA::ConstantDescription>(std::move(value), _tc_ConstantDescription); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::ConstantDescription& value) { return any._extract<::CORBA::ConstantDescription>(value); }

    struct TypeDescription {
        ::CORBA::Identifier name;
        ::CORBA::RepositoryId id;
        ::CORBA::RepositoryId defined_in;
        ::CORBA::VersionSpec version;
        ::omg::types::ref_type<::CORBA::TypeCode> type;
    };
    inline bool operator==(const TypeDescription& a, const TypeDescription& b) {
        return a.name == b.name
            && a.id == b.id
            && a.defined_in == b.defined_in
            && a.version == b.version
            && a.type == b.type;
    }
    inline bool operator!=(const TypeDescription& a, const TypeDescription& b) {
        return !(a == b);
    }
    inline void swap(TypeDescription& a, TypeDescription& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.id, b.id);
        swap(a.defined_in, b.defined_in);
        swap(a.version, b.version);
        swap(a.type, b.type);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_TypeDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/TypeDescription:1.0", "TypeDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::TypeDescription& value) { any._insert<::CORBA::TypeDescription>(value, _tc_TypeDescription); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::TypeDescription&& value) { any._insert<::CORBA::TypeDescription>(std::move(value), _tc_TypeDescription); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::TypeDescription& value) { return any._extract<::CORBA::TypeDescription>(value); }

    struct AttributeDescription {
        ::CORBA::Identifier name;
        ::CORBA::RepositoryId id;
        ::CORBA::RepositoryId defined_in;
        ::CORBA::VersionSpec version;
        ::omg::types::ref_type<::CORBA::TypeCode> type;
        ::CORBA::AttributeMode mode {::CORBA::AttributeMode::ATTR_NORMAL};
    };
    inline bool operator==(const AttributeDescription& a, const AttributeDescription& b) {
        return a.name == b.name
            && a.id == b.id
            && a.defined_in == b.defined_in
            && a.version == b.version
            && a.type == b.type
            && a.mode == b.mode;
    }
    inline bool operator!=(const AttributeDescription& a, const AttributeDescription& b) {
        return !(a == b);
    }
    inline void swap(AttributeDescription& a, AttributeDescription& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.id, b.id);
        swap(a.defined_in, b.defined_in);
        swap(a.version, b.version);
        swap(a.type, b.type);
        swap(a.mode, b.mode);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_AttributeDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/AttributeDescription:1.0", "AttributeDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"mode", ::CORBA::_tc_AttributeMode, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::AttributeDescription& value) { any._insert<::CORBA::AttributeDescription>(value, _tc_AttributeDescription); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::AttributeDescription&& value) { any._insert<::CORBA::AttributeDescription>(std::move(value), _tc_AttributeDescription); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::AttributeDescription& value) { return any._extract<::CORBA::AttributeDescription>(value); }

    struct ExtAttributeDescription {
        ::CORBA::Identifier name;
        ::CORBA::RepositoryId id;
        ::CORBA::RepositoryId defined_in;
        ::CORBA::VersionSpec version;
        ::omg::types::ref_type<::CORBA::TypeCode> type;
        ::CORBA::AttributeMode mode {::CORBA::AttributeMode::ATTR_NORMAL};
        ::CORBA::ExcDescriptionSeq get_exceptions;
        ::CORBA::ExcDescriptionSeq put_exceptions;
    };
    inline bool operator==(const ExtAttributeDescription& a, const ExtAttributeDescription& b) {
        return a.name == b.name
            && a.id == b.id
            && a.defined_in == b.defined_in
            && a.version == b.version
            && a.type == b.type
            && a.mode == b.mode
            && a.get_exceptions == b.get_exceptions
            && a.put_exceptions == b.put_exceptions;
    }
    inline bool operator!=(const ExtAttributeDescription& a, const ExtAttributeDescription& b) {
        return !(a == b);
    }
    inline void swap(ExtAttributeDescription& a, ExtAttributeDescription& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.id, b.id);
        swap(a.defined_in, b.defined_in);
        swap(a.version, b.version);
        swap(a.type, b.type);
        swap(a.mode, b.mode);
        swap(a.get_exceptions, b.get_exceptions);
        swap(a.put_exceptions, b.put_exceptions);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtAttributeDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ExtAttributeDescription:1.0", "ExtAttributeDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"mode", ::CORBA::_tc_AttributeMode, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"get_exceptions", ::CORBA::_tc_ExcDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"put_exceptions", ::CORBA::_tc_ExcDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::ExtAttributeDescription& value) { any._insert<::CORBA::ExtAttributeDescription>(value, _tc_ExtAttributeDescription); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::ExtAttributeDescription&& value) { any._insert<::CORBA::ExtAttributeDescription>(std::move(value), _tc_ExtAttributeDescription); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::ExtAttributeDescription& value) { return any._extract<::CORBA::ExtAttributeDescription>(value); }

    class ExtAttributeDef : public virtual ::CORBA::AttributeDef {
        public:

        virtual ~ExtAttributeDef() = default;

        virtual void get_exceptions(const ::CORBA::ExcDescriptionSeq& value) = 0;
        virtual ::CORBA::ExcDescriptionSeq get_exceptions() const = 0;

        virtual void set_exceptions(const ::CORBA::ExcDescriptionSeq& value) = 0;
        virtual ::CORBA::ExcDescriptionSeq set_exceptions() const = 0;

        virtual ::CORBA::ExtAttributeDescription describe_attribute() = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ExtAttributeDef:1.0" || ::CORBA::AttributeDef::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ExtAttributeDef:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ExtAttributeDef:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ExtAttributeDef = ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/ExtAttributeDef:1.0", "ExtAttributeDef");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ExtAttributeDef> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ExtAttributeDef); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ExtAttributeDef>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ExtAttributeDef>(base); return !base || value; }

    enum class ParameterMode {
        PARAM_IN = 0,
        PARAM_OUT = 1,
        PARAM_INOUT = 2
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ParameterMode = ::CORBA::TypeCode::create_enum_tc("IDL:omg.org/CORBA/ParameterMode:1.0", "ParameterMode", {"PARAM_IN", "PARAM_OUT", "PARAM_INOUT"});
    inline void operator<<=(::CORBA::Any& any, ::CORBA::ParameterMode value) { any._insert<::CORBA::ParameterMode>(value, _tc_ParameterMode); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::ParameterMode& value) { return any._extract<::CORBA::ParameterMode>(value); }

    struct ParameterDescription {
        ::CORBA::Identifier name;
        ::omg::types::ref_type<::CORBA::TypeCode> type;
        ::omg::types::ref_type<::CORBA::IDLType> type_def;
        ::CORBA::ParameterMode mode {::CORBA::ParameterMode::PARAM_IN};
    };
    inline bool operator==(const ParameterDescription& a, const ParameterDescription& b) {
        return a.name == b.name
            && a.type == b.type
            && a.type_def == b.type_def
            && a.mode == b.mode;
    }
    inline bool operator!=(const ParameterDescription& a, const ParameterDescription& b) {
        return !(a == b);
    }
    inline void swap(ParameterDescription& a, ParameterDescription& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.type, b.type);
        swap(a.type_def, b.type_def);
        swap(a.mode, b.mode);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ParameterDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ParameterDescription:1.0", "ParameterDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type_def", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/IDLType:1.0", "IDLType"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"mode", ::CORBA::_tc_ParameterMode, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::ParameterDescription& value) { any._insert<::CORBA::ParameterDescription>(value, _tc_ParameterDescription); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::ParameterDescription&& value) { any._insert<::CORBA::ParameterDescription>(std::move(value), _tc_ParameterDescription); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::ParameterDescription& value) { return any._extract<::CORBA::ParameterDescription>(value); }

    struct OperationDescription {
        ::CORBA::Identifier name;
        ::CORBA::RepositoryId id;
        ::CORBA::RepositoryId defined_in;
        ::CORBA::VersionSpec version;
        ::omg::types::ref_type<::CORBA::TypeCode> result;
        ::CORBA::OperationMode mode {::CORBA::OperationMode::OP_NORMAL};
        ::CORBA::ContextIdSeq contexts;
        ::CORBA::ParDescriptionSeq parameters;
        ::CORBA::ExcDescriptionSeq exceptions;
    };
    inline bool operator==(const OperationDescription& a, const OperationDescription& b) {
        return a.name == b.name
            && a.id == b.id
            && a.defined_in == b.defined_in
            && a.version == b.version
            && a.result == b.result
            && a.mode == b.mode
            && a.contexts == b.contexts
            && a.parameters == b.parameters
            && a.exceptions == b.exceptions;
    }
    inline bool operator!=(const OperationDescription& a, const OperationDescription& b) {
        return !(a == b);
    }
    inline void swap(OperationDescription& a, OperationDescription& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.id, b.id);
        swap(a.defined_in, b.defined_in);
        swap(a.version, b.version);
        swap(a.result, b.result);
        swap(a.mode, b.mode);
        swap(a.contexts, b.contexts);
        swap(a.parameters, b.parameters);
        swap(a.exceptions, b.exceptions);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_OperationDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/OperationDescription:1.0", "OperationDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"result", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"mode", ::CORBA::_tc_OperationMode, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"contexts", ::CORBA::_tc_ContextIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"parameters", ::CORBA::_tc_ParDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"exceptions", ::CORBA::_tc_ExcDescriptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::OperationDescription& value) { any._insert<::CORBA::OperationDescription>(value, _tc_OperationDescription); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::OperationDescription&& value) { any._insert<::CORBA::OperationDescription>(std::move(value), _tc_OperationDescription); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::OperationDescription& value) { return any._extract<::CORBA::OperationDescription>(value); }

    struct InterfaceDescription {
        ::CORBA::Identifier name;
        ::CORBA::RepositoryId id;
        ::CORBA::RepositoryId defined_in;
        ::CORBA::VersionSpec version;
        ::CORBA::RepositoryIdSeq base_interfaces;
        bool is_abstract {};
    };
    inline bool operator==(const InterfaceDescription& a, const InterfaceDescription& b) {
        return a.name == b.name
            && a.id == b.id
            && a.defined_in == b.defined_in
            && a.version == b.version
            && a.base_interfaces == b.base_interfaces
            && a.is_abstract == b.is_abstract;
    }
    inline bool operator!=(const InterfaceDescription& a, const InterfaceDescription& b) {
        return !(a == b);
    }
    inline void swap(InterfaceDescription& a, InterfaceDescription& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.id, b.id);
        swap(a.defined_in, b.defined_in);
        swap(a.version, b.version);
        swap(a.base_interfaces, b.base_interfaces);
        swap(a.is_abstract, b.is_abstract);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_InterfaceDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/InterfaceDescription:1.0", "InterfaceDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"base_interfaces", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_abstract", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::InterfaceDescription& value) { any._insert<::CORBA::InterfaceDescription>(value, _tc_InterfaceDescription); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::InterfaceDescription&& value) { any._insert<::CORBA::InterfaceDescription>(std::move(value), _tc_InterfaceDescription); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::InterfaceDescription& value) { return any._extract<::CORBA::InterfaceDescription>(value); }

    struct ValueMember {
        ::CORBA::Identifier name;
        ::CORBA::RepositoryId id;
        ::CORBA::RepositoryId defined_in;
        ::CORBA::VersionSpec version;
        ::omg::types::ref_type<::CORBA::TypeCode> type;
        ::omg::types::ref_type<::CORBA::IDLType> type_def;
        ::CORBA::Visibility access {};
    };
    inline bool operator==(const ValueMember& a, const ValueMember& b) {
        return a.name == b.name
            && a.id == b.id
            && a.defined_in == b.defined_in
            && a.version == b.version
            && a.type == b.type
            && a.type_def == b.type_def
            && a.access == b.access;
    }
    inline bool operator!=(const ValueMember& a, const ValueMember& b) {
        return !(a == b);
    }
    inline void swap(ValueMember& a, ValueMember& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.id, b.id);
        swap(a.defined_in, b.defined_in);
        swap(a.version, b.version);
        swap(a.type, b.type);
        swap(a.type_def, b.type_def);
        swap(a.access, b.access);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ValueMember = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ValueMember:1.0", "ValueMember", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/TypeCode:1.0", "TypeCode"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"type_def", ::CORBA::TypeCode::create_interface_tc("IDL:omg.org/CORBA/IDLType:1.0", "IDLType"), nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"access", ::CORBA::TypeCode::create_recursive_tc("IDL:omg.org/CORBA/Visibility:1.0"), nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::ValueMember& value) { any._insert<::CORBA::ValueMember>(value, _tc_ValueMember); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::ValueMember&& value) { any._insert<::CORBA::ValueMember>(std::move(value), _tc_ValueMember); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::ValueMember& value) { return any._extract<::CORBA::ValueMember>(value); }

    struct ValueDescription {
        ::CORBA::Identifier name;
        ::CORBA::RepositoryId id;
        bool is_abstract {};
        bool is_custom {};
        ::CORBA::RepositoryId defined_in;
        ::CORBA::VersionSpec version;
        ::CORBA::RepositoryIdSeq supported_interfaces;
        ::CORBA::RepositoryIdSeq abstract_base_values;
        bool is_truncatable {};
        ::CORBA::RepositoryId base_value;
    };
    inline bool operator==(const ValueDescription& a, const ValueDescription& b) {
        return a.name == b.name
            && a.id == b.id
            && a.is_abstract == b.is_abstract
            && a.is_custom == b.is_custom
            && a.defined_in == b.defined_in
            && a.version == b.version
            && a.supported_interfaces == b.supported_interfaces
            && a.abstract_base_values == b.abstract_base_values
            && a.is_truncatable == b.is_truncatable
            && a.base_value == b.base_value;
    }
    inline bool operator!=(const ValueDescription& a, const ValueDescription& b) {
        return !(a == b);
    }
    inline void swap(ValueDescription& a, ValueDescription& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.id, b.id);
        swap(a.is_abstract, b.is_abstract);
        swap(a.is_custom, b.is_custom);
        swap(a.defined_in, b.defined_in);
        swap(a.version, b.version);
        swap(a.supported_interfaces, b.supported_interfaces);
        swap(a.abstract_base_values, b.abstract_base_values);
        swap(a.is_truncatable, b.is_truncatable);
        swap(a.base_value, b.base_value);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ValueDescription = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ValueDescription:1.0", "ValueDescription", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"id", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_abstract", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_custom", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"defined_in", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"version", ::CORBA::_tc_VersionSpec, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"supported_interfaces", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"abstract_base_values", ::CORBA::_tc_RepositoryIdSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"is_truncatable", ::CORBA::_tc_boolean, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"base_value", ::CORBA::_tc_RepositoryId, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::ValueDescription& value) { any._insert<::CORBA::ValueDescription>(value, _tc_ValueDescription); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::ValueDescription&& value) { any._insert<::CORBA::ValueDescription>(std::move(value), _tc_ValueDescription); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::ValueDescription& value) { return any._extract<::CORBA::ValueDescription>(value); }

    using StringSeq = ::omg::types::sequence<std::string>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_StringSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/StringSeq:1.0", "StringSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_string_tc(0)));

    using WStringSeq = ::omg::types::sequence<std::wstring>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_WStringSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/WStringSeq:1.0", "WStringSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_wstring_tc(0)));

    class BadFixedValue : public ::CORBA::UserException {
        public:
        BadFixedValue() : offset_(), what_("BadFixedValue") {}
        BadFixedValue(const BadFixedValue&) = default;
        BadFixedValue(BadFixedValue&&) = default;
        explicit BadFixedValue(std::uint32_t offset, const char* what) : offset_(offset), what_(what ? what : "") {}
        ~BadFixedValue() noexcept override = default;
        BadFixedValue& operator=(const BadFixedValue&) = default;
        BadFixedValue& operator=(BadFixedValue&&) = default;

        void offset(std::uint32_t value) { offset_ = value; }
        std::uint32_t offset() const { return offset_; }

        void raise() const override { throw *this; }
        const char* _name() const override { return "BadFixedValue"; }
        const char* _rep_id() const override { return "IDL:omg.org/CORBA/BadFixedValue:1.0"; }
        void _insert_into(::CORBA::Any& any) const override;

        const char* what() const noexcept override { return what_.c_str(); }

        private:
        std::uint32_t offset_;
        std::string what_;
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_BadFixedValue = ::CORBA::TypeCode::create_exception_tc("IDL:omg.org/CORBA/BadFixedValue:1.0", "BadFixedValue", {::CORBA::TypeCode::Member{"offset", ::CORBA::_tc_ulong, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::BadFixedValue& value) { any._insert<::CORBA::BadFixedValue>(value, _tc_BadFixedValue); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::BadFixedValue&& value) { any._insert<::CORBA::BadFixedValue>(std::move(value), _tc_BadFixedValue); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::BadFixedValue& value) { return any._extract<::CORBA::BadFixedValue>(value); }
    inline void ::CORBA::BadFixedValue::_insert_into(::CORBA::Any& any) const { any <<= *this; }

    class WrongTransaction : public ::CORBA::UserException {
        public:
        WrongTransaction() : what_("WrongTransaction") {}
        WrongTransaction(const WrongTransaction&) = default;
        WrongTransaction(WrongTransaction&&) = default;
        explicit WrongTransaction(const char* what) : what_(what ? what : "") {}
        ~WrongTransaction() noexcept override = default;
        WrongTransaction& operator=(const WrongTransaction&) = default;
        WrongTransaction& operator=(WrongTransaction&&) = default;

        void raise() const override { throw *this; }
        const char* _name() const override { return "WrongTransaction"; }
        const char* _rep_id() const override { return "IDL:omg.org/CORBA/WrongTransaction:1.0"; }
        void _insert_into(::CORBA::Any& any) const override;

        const char* what() const noexcept override { return what_.c_str(); }

        private:
        std::string what_;
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_WrongTransaction = ::CORBA::TypeCode::create_exception_tc("IDL:omg.org/CORBA/WrongTransaction:1.0", "WrongTransaction", {});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::WrongTransaction& value) { any._insert<::CORBA::WrongTransaction>(value, _tc_WrongTransaction); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::WrongTransaction&& value) { any._insert<::CORBA::WrongTransaction>(std::move(value), _tc_WrongTransaction); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::WrongTransaction& value) { return any._extract<::CORBA::WrongTransaction>(value); }
    inline void ::CORBA::WrongTransaction::_insert_into(::CORBA::Any& any) const { any <<= *this; }

    using Flags = std::uint32_t;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Flags = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/Flags:1.0", "Flags", ::CORBA::_tc_ulong);

    struct NamedValue {
        ::CORBA::Identifier name;
        ::CORBA::Any argument;
        std::int32_t len {};
        ::CORBA::Flags arg_modes {};
    };
    inline bool operator==(const NamedValue& a, const NamedValue& b) {
        return a.name == b.name
            && a.argument == b.argument
            && a.len == b.len
            && a.arg_modes == b.arg_modes;
    }
    inline bool operator!=(const NamedValue& a, const NamedValue& b) {
        return !(a == b);
    }
    inline void swap(NamedValue& a, NamedValue& b) {
        using std::swap;
        swap(a.name, b.name);
        swap(a.argument, b.argument);
        swap(a.len, b.len);
        swap(a.arg_modes, b.arg_modes);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_NamedValue = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/NamedValue:1.0", "NamedValue", {::CORBA::TypeCode::Member{"name", ::CORBA::_tc_Identifier, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"argument", ::CORBA::_tc_any, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"len", ::CORBA::_tc_long, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"arg_modes", ::CORBA::_tc_Flags, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::NamedValue& value) { any._insert<::CORBA::NamedValue>(value, _tc_NamedValue); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::NamedValue&& value) { any._insert<::CORBA::NamedValue>(std::move(value), _tc_NamedValue); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::NamedValue& value) { return any._extract<::CORBA::NamedValue>(value); }

    class NVList : public virtual ::CORBA::LocalObject {
        public:

        virtual ~NVList() = default;

        virtual void add_item(const ::CORBA::Identifier& item_name, ::omg::types::ref_type<::CORBA::TypeCode> item_type, const ::CORBA::OpaqueValue& value, std::int32_t value_len, ::CORBA::Flags item_flags) = 0;

        virtual void free() = 0;

        virtual void free_memory() = 0;

        virtual void get_count(std::int32_t& count) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/NVList:1.0" || ::CORBA::LocalObject::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/NVList:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/NVList:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_NVList = ::CORBA::TypeCode::create_local_interface_tc("IDL:omg.org/CORBA/NVList:1.0", "NVList");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::NVList> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_NVList); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::NVList>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::NVList>(base); return !base || value; }

    class Context : public virtual ::CORBA::LocalObject {
        public:

        virtual ~Context() = default;

        virtual void set_one_value(const ::CORBA::Identifier& prop_name, const std::string& value) = 0;

        virtual void set_values(::omg::types::ref_type<::CORBA::NVList> values) = 0;

        virtual void get_values(const ::CORBA::Identifier& start_scope, ::CORBA::Flags op_flags, const ::CORBA::Identifier& prop_name, ::omg::types::ref_type<::CORBA::NVList>& values) = 0;

        virtual void delete_values(const ::CORBA::Identifier& prop_name) = 0;

        virtual void create_child(const ::CORBA::Identifier& ctx_name, ::omg::types::ref_type<::CORBA::Context>& child_ctx) = 0;

        virtual void _delete(::CORBA::Flags del_flags) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/Context:1.0" || ::CORBA::LocalObject::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/Context:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/Context:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Context = ::CORBA::TypeCode::create_local_interface_tc("IDL:omg.org/CORBA/Context:1.0", "Context");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Context> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_Context); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Context>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::Context>(base); return !base || value; }

    class Request : public virtual ::CORBA::LocalObject {
        public:

        virtual ~Request() = default;

        virtual void add_arg(const ::CORBA::Identifier& name, ::omg::types::ref_type<::CORBA::TypeCode> arg_type, const ::CORBA::OpaqueValue& value, std::int32_t len, ::CORBA::Flags arg_flags) = 0;

        virtual void invoke(::CORBA::Flags invoke_flags) = 0;

        virtual void _delete() = 0;

        virtual void send(::CORBA::Flags invoke_flags) = 0;

        virtual void get_response() = 0;

        virtual bool poll_response() = 0;

        virtual ::omg::types::ref_type<::CORBA::Object> sendp() = 0;

        virtual void prepare(::omg::types::ref_type<::CORBA::Object> p) = 0;

        virtual void sendc(::omg::types::ref_type<::CORBA::Object> handler) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/Request:1.0" || ::CORBA::LocalObject::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/Request:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/Request:1.0"; }
    };

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_Request = ::CORBA::TypeCode::create_local_interface_tc("IDL:omg.org/CORBA/Request:1.0", "Request");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Request> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_Request); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::Request>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::Request>(base); return !base || value; }

    using RequestSeq = ::omg::types::sequence<::omg::types::ref_type<::CORBA::Request>>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_RequestSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/RequestSeq:1.0", "RequestSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::TypeCode::create_local_interface_tc("IDL:omg.org/CORBA/Request:1.0", "Request")));

    using ServiceType = std::uint16_t;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ServiceType = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ServiceType:1.0", "ServiceType", ::CORBA::_tc_ushort);

    using ORBid = std::string;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ORBid = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ORBid:1.0", "ORBid", ::CORBA::TypeCode::create_string_tc(0));

    class ORB : public virtual ::CORBA::LocalObject {
        public:

        using ObjectId = std::string;
        #if __cplusplus >= 201703L
        static inline const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ObjectId = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ORB/ObjectId:1.0", "ObjectId", ::CORBA::TypeCode::create_string_tc(0));
        #else
        static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ObjectId;
        #endif

        using ObjectIdList = ::omg::types::sequence<::CORBA::ORB::ObjectId>;
        #if __cplusplus >= 201703L
        static inline const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ObjectIdList = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ORB/ObjectIdList:1.0", "ObjectIdList", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::ORB::_tc_ObjectId));
        #else
        static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ObjectIdList;
        #endif

        class InvalidName : public ::CORBA::UserException {
            public:
            InvalidName() : what_("InvalidName") {}
            InvalidName(const InvalidName&) = default;
            InvalidName(InvalidName&&) = default;
            explicit InvalidName(const char* what) : what_(what ? what : "") {}
            ~InvalidName() noexcept override = default;
            InvalidName& operator=(const InvalidName&) = default;
            InvalidName& operator=(InvalidName&&) = default;

            void raise() const override { throw *this; }
            const char* _name() const override { return "InvalidName"; }
            const char* _rep_id() const override { return "IDL:omg.org/CORBA/ORB/InvalidName:1.0"; }
            void _insert_into(::CORBA::Any& any) const override;

            const char* what() const noexcept override { return what_.c_str(); }

            private:
            std::string what_;
        };
        #if __cplusplus >= 201703L
        static inline const ::omg::types::ref_type<::CORBA::TypeCode> _tc_InvalidName = ::CORBA::TypeCode::create_exception_tc("IDL:omg.org/CORBA/ORB/InvalidName:1.0", "InvalidName", {});
        #else
        static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_InvalidName;
        #endif
        friend void operator<<=(::CORBA::Any& any, const ::CORBA::ORB::InvalidName& value) { any._insert<::CORBA::ORB::InvalidName>(value, _tc_InvalidName); }
        friend void operator<<=(::CORBA::Any& any, ::CORBA::ORB::InvalidName&& value) { any._insert<::CORBA::ORB::InvalidName>(std::move(value), _tc_InvalidName); }
        friend bool operator>>=(const ::CORBA::Any& any, ::CORBA::ORB::InvalidName& value) { return any._extract<::CORBA::ORB::InvalidName>(value); }

        virtual ~ORB() = default;

        virtual ::CORBA::ORBid id() = 0;

        virtual std::string object_to_string(::omg::types::ref_type<::CORBA::Object> obj) = 0;

        virtual ::omg::types::ref_type<::CORBA::Object> string_to_object(const std::string& str) = 0;

        virtual void create_list(std::int32_t count, ::omg::types::ref_type<::CORBA::NVList>& new_list) = 0;

        virtual void create_operation_list(::omg::types::ref_type<::CORBA::OperationDef> oper, ::omg::types::ref_type<::CORBA::NVList>& new_list) = 0;

        virtual void get_default_context(::omg::types::ref_type<::CORBA::Context>& ctx) = 0;

        virtual void send_multiple_requests_oneway(const ::CORBA::RequestSeq& req) = 0;

        virtual void send_multiple_requests_deferred(const ::CORBA::RequestSeq& req) = 0;

        virtual bool poll_next_response() = 0;

        virtual void get_next_response(::omg::types::ref_type<::CORBA::Request>& req) = 0;

        virtual bool get_service_information(::CORBA::ServiceType service_type, ::CORBA::ServiceInformation& service_information) = 0;

        virtual ::CORBA::ORB::ObjectIdList list_initial_services() = 0;

        virtual ::omg::types::ref_type<::CORBA::Object> resolve_initial_references(const ::CORBA::ORB::ObjectId& identifier) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_struct_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::StructMemberSeq& members) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_union_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, ::omg::types::ref_type<::CORBA::TypeCode> discriminator_type, const ::CORBA::UnionMemberSeq& members) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_enum_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::EnumMemberSeq& members) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_alias_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, ::omg::types::ref_type<::CORBA::TypeCode> original_type) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_exception_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, const ::CORBA::StructMemberSeq& members) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_interface_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_string_tc(std::uint32_t bound) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_wstring_tc(std::uint32_t bound) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_fixed_tc(std::uint16_t digits, std::int16_t scale) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_sequence_tc(std::uint32_t bound, ::omg::types::ref_type<::CORBA::TypeCode> element_type) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_recursive_sequence_tc(std::uint32_t bound, std::uint32_t offset) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_array_tc(std::uint32_t length, ::omg::types::ref_type<::CORBA::TypeCode> element_type) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_value_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, ::CORBA::ValueModifier type_modifier, ::omg::types::ref_type<::CORBA::TypeCode> concrete_base, const ::CORBA::ValueMemberSeq& members) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_value_box_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, ::omg::types::ref_type<::CORBA::TypeCode> boxed_type) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_native_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_recursive_tc(const ::CORBA::RepositoryId& id) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_abstract_interface_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_local_interface_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_component_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_home_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name) = 0;

        virtual ::omg::types::ref_type<::CORBA::TypeCode> create_event_tc(const ::CORBA::RepositoryId& id, const ::CORBA::Identifier& name, ::CORBA::ValueModifier type_modifier, ::omg::types::ref_type<::CORBA::TypeCode> concrete_base, const ::CORBA::ValueMemberSeq& members) = 0;

        virtual bool work_pending() = 0;

        virtual void perform_work() = 0;

        virtual void run() = 0;

        virtual void shutdown(bool wait_for_completion) = 0;

        virtual void destroy() = 0;

        virtual ::omg::types::ref_type<::CORBA::Policy> create_policy(::CORBA::PolicyType type, const ::CORBA::Any& val) = 0;

        virtual ::omg::types::ref_type<::CORBA::ValueFactoryBase> register_value_factory(const ::CORBA::RepositoryId& id, ::omg::types::ref_type<::CORBA::ValueFactoryBase> factory) = 0;

        virtual void unregister_value_factory(const ::CORBA::RepositoryId& id) = 0;

        virtual ::omg::types::ref_type<::CORBA::ValueFactoryBase> lookup_value_factory(const ::CORBA::RepositoryId& id) = 0;

        virtual void register_initial_reference(const ::CORBA::ORB::ObjectId& id, ::omg::types::ref_type<::CORBA::Object> obj) = 0;

        bool _is_a(const std::string& logical_type_id) override { return logical_type_id == "IDL:omg.org/CORBA/ORB:1.0" || ::CORBA::LocalObject::_is_a(logical_type_id); }
        std::string _repository_id() override { return "IDL:omg.org/CORBA/ORB:1.0"; }
        static const char* _interface_repository_id() { return "IDL:omg.org/CORBA/ORB:1.0"; }
    };
    #if __cplusplus < 201703L
    const ::omg::types::ref_type<::CORBA::TypeCode> CORBA::ORB::_tc_ObjectId = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ORB/ObjectId:1.0", "ObjectId", ::CORBA::TypeCode::create_string_tc(0));
    #endif
    #if __cplusplus < 201703L
    const ::omg::types::ref_type<::CORBA::TypeCode> CORBA::ORB::_tc_ObjectIdList = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ORB/ObjectIdList:1.0", "ObjectIdList", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::ORB::_tc_ObjectId));
    #endif
    #if __cplusplus < 201703L
    const ::omg::types::ref_type<::CORBA::TypeCode> CORBA::ORB::_tc_InvalidName = ::CORBA::TypeCode::create_exception_tc("IDL:omg.org/CORBA/ORB/InvalidName:1.0", "InvalidName", {});
    #endif
    inline void ::CORBA::ORB::InvalidName::_insert_into(::CORBA::Any& any) const { any <<= *this; }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ORB = ::CORBA::TypeCode::create_local_interface_tc("IDL:omg.org/CORBA/ORB:1.0", "ORB");
    inline void operator<<=(::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ORB> value) { any._insert<::omg::types::ref_type<::CORBA::Object>>(std::move(value), _tc_ORB); }
    inline bool operator>>=(const ::CORBA::Any& any, ::omg::types::ref_type<::CORBA::ORB>& value) { ::omg::types::ref_type<::CORBA::Object> base; if (!any._extract(base)) return false; value = std::dynamic_pointer_cast<::CORBA::ORB>(base); return !base || value; }

    using ServiceOption = std::uint32_t;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ServiceOption = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ServiceOption:1.0", "ServiceOption", ::CORBA::_tc_ulong);

    using ServiceDetailType = std::uint32_t;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ServiceDetailType = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ServiceDetailType:1.0", "ServiceDetailType", ::CORBA::_tc_ulong);

    using ServiceDetailData = ::CORBA::OctetSeq;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ServiceDetailData = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ServiceDetailData:1.0", "ServiceDetailData", ::CORBA::_tc_OctetSeq);

    using ServiceOptionSeq = ::omg::types::sequence<::CORBA::ServiceOption>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ServiceOptionSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ServiceOptionSeq:1.0", "ServiceOptionSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_ServiceOption));

    constexpr ::CORBA::ServiceType Security = 1;

    struct ServiceDetail {
        ::CORBA::ServiceDetailType service_detail_type {};
        ::CORBA::ServiceDetailData service_detail;
    };
    inline bool operator==(const ServiceDetail& a, const ServiceDetail& b) {
        return a.service_detail_type == b.service_detail_type
            && a.service_detail == b.service_detail;
    }
    inline bool operator!=(const ServiceDetail& a, const ServiceDetail& b) {
        return !(a == b);
    }
    inline void swap(ServiceDetail& a, ServiceDetail& b) {
        using std::swap;
        swap(a.service_detail_type, b.service_detail_type);
        swap(a.service_detail, b.service_detail);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ServiceDetail = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ServiceDetail:1.0", "ServiceDetail", {::CORBA::TypeCode::Member{"service_detail_type", ::CORBA::_tc_ServiceDetailType, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"service_detail", ::CORBA::_tc_ServiceDetailData, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::ServiceDetail& value) { any._insert<::CORBA::ServiceDetail>(value, _tc_ServiceDetail); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::ServiceDetail&& value) { any._insert<::CORBA::ServiceDetail>(std::move(value), _tc_ServiceDetail); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::ServiceDetail& value) { return any._extract<::CORBA::ServiceDetail>(value); }

    using ServiceDetailSeq = ::omg::types::sequence<::CORBA::ServiceDetail>;
    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ServiceDetailSeq = ::CORBA::TypeCode::create_alias_tc("IDL:omg.org/CORBA/ServiceDetailSeq:1.0", "ServiceDetailSeq", ::CORBA::TypeCode::create_sequence_tc(0, ::CORBA::_tc_ServiceDetail));

    struct ServiceInformation {
        ::CORBA::ServiceOptionSeq service_options;
        ::CORBA::ServiceDetailSeq service_details;
    };
    inline bool operator==(const ServiceInformation& a, const ServiceInformation& b) {
        return a.service_options == b.service_options
            && a.service_details == b.service_details;
    }
    inline bool operator!=(const ServiceInformation& a, const ServiceInformation& b) {
        return !(a == b);
    }
    inline void swap(ServiceInformation& a, ServiceInformation& b) {
        using std::swap;
        swap(a.service_options, b.service_options);
        swap(a.service_details, b.service_details);
    }

    static const ::omg::types::ref_type<::CORBA::TypeCode> _tc_ServiceInformation = ::CORBA::TypeCode::create_struct_tc("IDL:omg.org/CORBA/ServiceInformation:1.0", "ServiceInformation", {::CORBA::TypeCode::Member{"service_options", ::CORBA::_tc_ServiceOptionSeq, nullptr, ::CORBA::PUBLIC_MEMBER}, ::CORBA::TypeCode::Member{"service_details", ::CORBA::_tc_ServiceDetailSeq, nullptr, ::CORBA::PUBLIC_MEMBER}});
    inline void operator<<=(::CORBA::Any& any, const ::CORBA::ServiceInformation& value) { any._insert<::CORBA::ServiceInformation>(value, _tc_ServiceInformation); }
    inline void operator<<=(::CORBA::Any& any, ::CORBA::ServiceInformation&& value) { any._insert<::CORBA::ServiceInformation>(std::move(value), _tc_ServiceInformation); }
    inline bool operator>>=(const ::CORBA::Any& any, ::CORBA::ServiceInformation& value) { return any._extract<::CORBA::ServiceInformation>(value); }
} // namespace CORBA

namespace omg { namespace types {
    template <> struct is_idl_type<::CORBA::CustomMarshal> : std::true_type {};
    template <> struct is_reference_type<::CORBA::CustomMarshal> : std::true_type {};
    template <> struct is_idl_type<::CORBA::DataInputStream> : std::true_type {};
    template <> struct is_reference_type<::CORBA::DataInputStream> : std::true_type {};
    template <> struct is_idl_type<::CORBA::DataOutputStream> : std::true_type {};
    template <> struct is_reference_type<::CORBA::DataOutputStream> : std::true_type {};
    template <> struct is_idl_type<::CORBA::Pollable> : std::true_type {};
    template <> struct is_reference_type<::CORBA::Pollable> : std::true_type {};
    template <> struct is_idl_type<::CORBA::DIIPollable> : std::true_type {};
    template <> struct is_reference_type<::CORBA::DIIPollable> : std::true_type {};
    template <> struct is_idl_type<::CORBA::Contained::Description> : std::true_type {};
    template <> struct is_idl_type<::CORBA::StructMember> : std::true_type {};
    template <> struct is_idl_type<::CORBA::Initializer> : std::true_type {};
    template <> struct is_idl_type<::CORBA::UnionMember> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ExceptionDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ExtInitializer> : std::true_type {};
    template <> struct is_idl_type<::CORBA::Container::Description> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ModuleDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ConstantDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::TypeDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::AttributeDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ExtAttributeDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ParameterDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::OperationDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::InterfaceDef::FullInterfaceDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::InterfaceDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::InterfaceAttrExtension::ExtFullInterfaceDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ValueMember> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ValueDef::FullValueDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ValueDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ExtValueDef::ExtFullValueDescription> : std::true_type {};
    template <> struct is_idl_type<::CORBA::NamedValue> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ServiceDetail> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ServiceInformation> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ConstructionPolicy> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ConstructionPolicy> : std::true_type {};
    template <> struct is_idl_type<::CORBA::Current> : std::true_type {};
    template <> struct is_reference_type<::CORBA::Current> : std::true_type {};
    template <> struct is_idl_type<::CORBA::DomainManager> : std::true_type {};
    template <> struct is_reference_type<::CORBA::DomainManager> : std::true_type {};
    template <> struct is_idl_type<::CORBA::Policy> : std::true_type {};
    template <> struct is_reference_type<::CORBA::Policy> : std::true_type {};
    template <> struct is_idl_type<::CORBA::PollableSet> : std::true_type {};
    template <> struct is_reference_type<::CORBA::PollableSet> : std::true_type {};
    template <> struct is_idl_type<::CORBA::AbstractInterfaceDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::AbstractInterfaceDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::AliasDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::AliasDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ArrayDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ArrayDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::AttributeDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::AttributeDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ConstantDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ConstantDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::Contained> : std::true_type {};
    template <> struct is_reference_type<::CORBA::Contained> : std::true_type {};
    template <> struct is_idl_type<::CORBA::Container> : std::true_type {};
    template <> struct is_reference_type<::CORBA::Container> : std::true_type {};
    template <> struct is_idl_type<::CORBA::EnumDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::EnumDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ExceptionDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ExceptionDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ExtInterfaceDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ExtInterfaceDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ExtValueDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ExtValueDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ExtAbstractInterfaceDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ExtAbstractInterfaceDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ExtLocalInterfaceDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ExtLocalInterfaceDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::FixedDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::FixedDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::IDLType> : std::true_type {};
    template <> struct is_reference_type<::CORBA::IDLType> : std::true_type {};
    template <> struct is_idl_type<::CORBA::InterfaceDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::InterfaceDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::IRObject> : std::true_type {};
    template <> struct is_reference_type<::CORBA::IRObject> : std::true_type {};
    template <> struct is_idl_type<::CORBA::LocalInterfaceDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::LocalInterfaceDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ModuleDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ModuleDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::NativeDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::NativeDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::OperationDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::OperationDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::PrimitiveDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::PrimitiveDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::Repository> : std::true_type {};
    template <> struct is_reference_type<::CORBA::Repository> : std::true_type {};
    template <> struct is_idl_type<::CORBA::SequenceDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::SequenceDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::StringDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::StringDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::StructDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::StructDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::TypedefDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::TypedefDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::UnionDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::UnionDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ValueDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ValueDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ValueBoxDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ValueBoxDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ValueMemberDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ValueMemberDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::WstringDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::WstringDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::PolicyManager> : std::true_type {};
    template <> struct is_reference_type<::CORBA::PolicyManager> : std::true_type {};
    template <> struct is_idl_type<::CORBA::PolicyCurrent> : std::true_type {};
    template <> struct is_reference_type<::CORBA::PolicyCurrent> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ExtAttributeDef> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ExtAttributeDef> : std::true_type {};
    template <> struct is_idl_type<::CORBA::InterfaceAttrExtension> : std::true_type {};
    template <> struct is_reference_type<::CORBA::InterfaceAttrExtension> : std::true_type {};
    template <> struct is_idl_type<::CORBA::NVList> : std::true_type {};
    template <> struct is_reference_type<::CORBA::NVList> : std::true_type {};
    template <> struct is_idl_type<::CORBA::Context> : std::true_type {};
    template <> struct is_reference_type<::CORBA::Context> : std::true_type {};
    template <> struct is_idl_type<::CORBA::Request> : std::true_type {};
    template <> struct is_reference_type<::CORBA::Request> : std::true_type {};
    template <> struct is_idl_type<::CORBA::ORB> : std::true_type {};
    template <> struct is_reference_type<::CORBA::ORB> : std::true_type {};
} } // namespace omg::types

namespace CORBA {
    template <> struct traits<::CORBA::CustomMarshal> : detail::reference_traits<::CORBA::CustomMarshal> {
        using is_abstract = std::true_type;
        using is_truncatable = std::false_type;
        using factory_type = ::CORBA::ValueFactoryBase;
        using base_type = ::CORBA::CustomMarshal;
        static ::omg::types::ref_type<::CORBA::CustomMarshal> narrow(::omg::types::ref_type<::CORBA::ValueBase> p) { return std::dynamic_pointer_cast<::CORBA::CustomMarshal>(p); }
    };
    template <> struct traits<::CORBA::DataInputStream> : detail::reference_traits<::CORBA::DataInputStream> {
        using is_abstract = std::true_type;
        using is_truncatable = std::false_type;
        using factory_type = ::CORBA::ValueFactoryBase;
        using base_type = ::CORBA::DataInputStream;
        static ::omg::types::ref_type<::CORBA::DataInputStream> narrow(::omg::types::ref_type<::CORBA::ValueBase> p) { return std::dynamic_pointer_cast<::CORBA::DataInputStream>(p); }
    };
    template <> struct traits<::CORBA::DataOutputStream> : detail::reference_traits<::CORBA::DataOutputStream> {
        using is_abstract = std::true_type;
        using is_truncatable = std::false_type;
        using factory_type = ::CORBA::ValueFactoryBase;
        using base_type = ::CORBA::DataOutputStream;
        static ::omg::types::ref_type<::CORBA::DataOutputStream> narrow(::omg::types::ref_type<::CORBA::ValueBase> p) { return std::dynamic_pointer_cast<::CORBA::DataOutputStream>(p); }
    };
    template <> struct traits<::CORBA::Pollable> : detail::reference_traits<::CORBA::Pollable> {
        using is_abstract = std::true_type;
        using is_truncatable = std::false_type;
        using factory_type = ::CORBA::ValueFactoryBase;
        using base_type = ::CORBA::Pollable;
        static ::omg::types::ref_type<::CORBA::Pollable> narrow(::omg::types::ref_type<::CORBA::ValueBase> p) { return std::dynamic_pointer_cast<::CORBA::Pollable>(p); }
    };
    template <> struct traits<::CORBA::DIIPollable> : detail::reference_traits<::CORBA::DIIPollable> {
        using is_abstract = std::true_type;
        using is_truncatable = std::false_type;
        using factory_type = ::CORBA::ValueFactoryBase;
        using base_type = ::CORBA::DIIPollable;
        static ::omg::types::ref_type<::CORBA::DIIPollable> narrow(::omg::types::ref_type<::CORBA::ValueBase> p) { return std::dynamic_pointer_cast<::CORBA::DIIPollable>(p); }
    };
    template <> struct traits<::CORBA::Contained::Description> : detail::common_traits<::CORBA::Contained::Description, false> {};
    template <> struct traits<::CORBA::StructMember> : detail::common_traits<::CORBA::StructMember, false> {};
    template <> struct traits<::CORBA::Initializer> : detail::common_traits<::CORBA::Initializer, false> {};
    template <> struct traits<::CORBA::UnionMember> : detail::common_traits<::CORBA::UnionMember, false> {};
    template <> struct traits<::CORBA::ExceptionDescription> : detail::common_traits<::CORBA::ExceptionDescription, false> {};
    template <> struct traits<::CORBA::ExtInitializer> : detail::common_traits<::CORBA::ExtInitializer, false> {};
    template <> struct traits<::CORBA::Container::Description> : detail::common_traits<::CORBA::Container::Description, false> {};
    template <> struct traits<::CORBA::ModuleDescription> : detail::common_traits<::CORBA::ModuleDescription, false> {};
    template <> struct traits<::CORBA::ConstantDescription> : detail::common_traits<::CORBA::ConstantDescription, false> {};
    template <> struct traits<::CORBA::TypeDescription> : detail::common_traits<::CORBA::TypeDescription, false> {};
    template <> struct traits<::CORBA::AttributeDescription> : detail::common_traits<::CORBA::AttributeDescription, false> {};
    template <> struct traits<::CORBA::ExtAttributeDescription> : detail::common_traits<::CORBA::ExtAttributeDescription, false> {};
    template <> struct traits<::CORBA::ParameterDescription> : detail::common_traits<::CORBA::ParameterDescription, false> {};
    template <> struct traits<::CORBA::OperationDescription> : detail::common_traits<::CORBA::OperationDescription, false> {};
    template <> struct traits<::CORBA::InterfaceDef::FullInterfaceDescription> : detail::common_traits<::CORBA::InterfaceDef::FullInterfaceDescription, false> {};
    template <> struct traits<::CORBA::InterfaceDescription> : detail::common_traits<::CORBA::InterfaceDescription, false> {};
    template <> struct traits<::CORBA::InterfaceAttrExtension::ExtFullInterfaceDescription> : detail::common_traits<::CORBA::InterfaceAttrExtension::ExtFullInterfaceDescription, false> {};
    template <> struct traits<::CORBA::ValueMember> : detail::common_traits<::CORBA::ValueMember, false> {};
    template <> struct traits<::CORBA::ValueDef::FullValueDescription> : detail::common_traits<::CORBA::ValueDef::FullValueDescription, false> {};
    template <> struct traits<::CORBA::ValueDescription> : detail::common_traits<::CORBA::ValueDescription, false> {};
    template <> struct traits<::CORBA::ExtValueDef::ExtFullValueDescription> : detail::common_traits<::CORBA::ExtValueDef::ExtFullValueDescription, false> {};
    template <> struct traits<::CORBA::NamedValue> : detail::common_traits<::CORBA::NamedValue, false> {};
    template <> struct traits<::CORBA::ServiceDetail> : detail::common_traits<::CORBA::ServiceDetail, false> {};
    template <> struct traits<::CORBA::ServiceInformation> : detail::common_traits<::CORBA::ServiceInformation, false> {};
    template <> struct traits<::CORBA::ConstructionPolicy> : detail::reference_traits<::CORBA::ConstructionPolicy> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ConstructionPolicy;
        static ::omg::types::ref_type<::CORBA::ConstructionPolicy> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ConstructionPolicy>(p); }
    };
    template <> struct traits<::CORBA::Current> : detail::reference_traits<::CORBA::Current> {
        using is_local = std::true_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::Current;
        static ::omg::types::ref_type<::CORBA::Current> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::Current>(p); }
    };
    template <> struct traits<::CORBA::DomainManager> : detail::reference_traits<::CORBA::DomainManager> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::DomainManager;
        static ::omg::types::ref_type<::CORBA::DomainManager> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::DomainManager>(p); }
    };
    template <> struct traits<::CORBA::Policy> : detail::reference_traits<::CORBA::Policy> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::Policy;
        static ::omg::types::ref_type<::CORBA::Policy> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::Policy>(p); }
    };
    template <> struct traits<::CORBA::PollableSet> : detail::reference_traits<::CORBA::PollableSet> {
        using is_local = std::true_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::PollableSet;
        static ::omg::types::ref_type<::CORBA::PollableSet> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::PollableSet>(p); }
    };
    template <> struct traits<::CORBA::AbstractInterfaceDef> : detail::reference_traits<::CORBA::AbstractInterfaceDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::AbstractInterfaceDef;
        static ::omg::types::ref_type<::CORBA::AbstractInterfaceDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::AbstractInterfaceDef>(p); }
    };
    template <> struct traits<::CORBA::AliasDef> : detail::reference_traits<::CORBA::AliasDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::AliasDef;
        static ::omg::types::ref_type<::CORBA::AliasDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::AliasDef>(p); }
    };
    template <> struct traits<::CORBA::ArrayDef> : detail::reference_traits<::CORBA::ArrayDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ArrayDef;
        static ::omg::types::ref_type<::CORBA::ArrayDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ArrayDef>(p); }
    };
    template <> struct traits<::CORBA::AttributeDef> : detail::reference_traits<::CORBA::AttributeDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::AttributeDef;
        static ::omg::types::ref_type<::CORBA::AttributeDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::AttributeDef>(p); }
    };
    template <> struct traits<::CORBA::ConstantDef> : detail::reference_traits<::CORBA::ConstantDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ConstantDef;
        static ::omg::types::ref_type<::CORBA::ConstantDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ConstantDef>(p); }
    };
    template <> struct traits<::CORBA::Contained> : detail::reference_traits<::CORBA::Contained> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::Contained;
        static ::omg::types::ref_type<::CORBA::Contained> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::Contained>(p); }
    };
    template <> struct traits<::CORBA::Container> : detail::reference_traits<::CORBA::Container> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::Container;
        static ::omg::types::ref_type<::CORBA::Container> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::Container>(p); }
    };
    template <> struct traits<::CORBA::EnumDef> : detail::reference_traits<::CORBA::EnumDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::EnumDef;
        static ::omg::types::ref_type<::CORBA::EnumDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::EnumDef>(p); }
    };
    template <> struct traits<::CORBA::ExceptionDef> : detail::reference_traits<::CORBA::ExceptionDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ExceptionDef;
        static ::omg::types::ref_type<::CORBA::ExceptionDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ExceptionDef>(p); }
    };
    template <> struct traits<::CORBA::ExtInterfaceDef> : detail::reference_traits<::CORBA::ExtInterfaceDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ExtInterfaceDef;
        static ::omg::types::ref_type<::CORBA::ExtInterfaceDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ExtInterfaceDef>(p); }
    };
    template <> struct traits<::CORBA::ExtValueDef> : detail::reference_traits<::CORBA::ExtValueDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ExtValueDef;
        static ::omg::types::ref_type<::CORBA::ExtValueDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ExtValueDef>(p); }
    };
    template <> struct traits<::CORBA::ExtAbstractInterfaceDef> : detail::reference_traits<::CORBA::ExtAbstractInterfaceDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ExtAbstractInterfaceDef;
        static ::omg::types::ref_type<::CORBA::ExtAbstractInterfaceDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ExtAbstractInterfaceDef>(p); }
    };
    template <> struct traits<::CORBA::ExtLocalInterfaceDef> : detail::reference_traits<::CORBA::ExtLocalInterfaceDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ExtLocalInterfaceDef;
        static ::omg::types::ref_type<::CORBA::ExtLocalInterfaceDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ExtLocalInterfaceDef>(p); }
    };
    template <> struct traits<::CORBA::FixedDef> : detail::reference_traits<::CORBA::FixedDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::FixedDef;
        static ::omg::types::ref_type<::CORBA::FixedDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::FixedDef>(p); }
    };
    template <> struct traits<::CORBA::IDLType> : detail::reference_traits<::CORBA::IDLType> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::IDLType;
        static ::omg::types::ref_type<::CORBA::IDLType> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::IDLType>(p); }
    };
    template <> struct traits<::CORBA::InterfaceDef> : detail::reference_traits<::CORBA::InterfaceDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::InterfaceDef;
        static ::omg::types::ref_type<::CORBA::InterfaceDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::InterfaceDef>(p); }
    };
    template <> struct traits<::CORBA::IRObject> : detail::reference_traits<::CORBA::IRObject> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::IRObject;
        static ::omg::types::ref_type<::CORBA::IRObject> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::IRObject>(p); }
    };
    template <> struct traits<::CORBA::LocalInterfaceDef> : detail::reference_traits<::CORBA::LocalInterfaceDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::LocalInterfaceDef;
        static ::omg::types::ref_type<::CORBA::LocalInterfaceDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::LocalInterfaceDef>(p); }
    };
    template <> struct traits<::CORBA::ModuleDef> : detail::reference_traits<::CORBA::ModuleDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ModuleDef;
        static ::omg::types::ref_type<::CORBA::ModuleDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ModuleDef>(p); }
    };
    template <> struct traits<::CORBA::NativeDef> : detail::reference_traits<::CORBA::NativeDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::NativeDef;
        static ::omg::types::ref_type<::CORBA::NativeDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::NativeDef>(p); }
    };
    template <> struct traits<::CORBA::OperationDef> : detail::reference_traits<::CORBA::OperationDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::OperationDef;
        static ::omg::types::ref_type<::CORBA::OperationDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::OperationDef>(p); }
    };
    template <> struct traits<::CORBA::PrimitiveDef> : detail::reference_traits<::CORBA::PrimitiveDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::PrimitiveDef;
        static ::omg::types::ref_type<::CORBA::PrimitiveDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::PrimitiveDef>(p); }
    };
    template <> struct traits<::CORBA::Repository> : detail::reference_traits<::CORBA::Repository> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::Repository;
        static ::omg::types::ref_type<::CORBA::Repository> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::Repository>(p); }
    };
    template <> struct traits<::CORBA::SequenceDef> : detail::reference_traits<::CORBA::SequenceDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::SequenceDef;
        static ::omg::types::ref_type<::CORBA::SequenceDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::SequenceDef>(p); }
    };
    template <> struct traits<::CORBA::StringDef> : detail::reference_traits<::CORBA::StringDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::StringDef;
        static ::omg::types::ref_type<::CORBA::StringDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::StringDef>(p); }
    };
    template <> struct traits<::CORBA::StructDef> : detail::reference_traits<::CORBA::StructDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::StructDef;
        static ::omg::types::ref_type<::CORBA::StructDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::StructDef>(p); }
    };
    template <> struct traits<::CORBA::TypedefDef> : detail::reference_traits<::CORBA::TypedefDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::TypedefDef;
        static ::omg::types::ref_type<::CORBA::TypedefDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::TypedefDef>(p); }
    };
    template <> struct traits<::CORBA::UnionDef> : detail::reference_traits<::CORBA::UnionDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::UnionDef;
        static ::omg::types::ref_type<::CORBA::UnionDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::UnionDef>(p); }
    };
    template <> struct traits<::CORBA::ValueDef> : detail::reference_traits<::CORBA::ValueDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ValueDef;
        static ::omg::types::ref_type<::CORBA::ValueDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ValueDef>(p); }
    };
    template <> struct traits<::CORBA::ValueBoxDef> : detail::reference_traits<::CORBA::ValueBoxDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ValueBoxDef;
        static ::omg::types::ref_type<::CORBA::ValueBoxDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ValueBoxDef>(p); }
    };
    template <> struct traits<::CORBA::ValueMemberDef> : detail::reference_traits<::CORBA::ValueMemberDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ValueMemberDef;
        static ::omg::types::ref_type<::CORBA::ValueMemberDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ValueMemberDef>(p); }
    };
    template <> struct traits<::CORBA::WstringDef> : detail::reference_traits<::CORBA::WstringDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::WstringDef;
        static ::omg::types::ref_type<::CORBA::WstringDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::WstringDef>(p); }
    };
    template <> struct traits<::CORBA::PolicyManager> : detail::reference_traits<::CORBA::PolicyManager> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::PolicyManager;
        static ::omg::types::ref_type<::CORBA::PolicyManager> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::PolicyManager>(p); }
    };
    template <> struct traits<::CORBA::PolicyCurrent> : detail::reference_traits<::CORBA::PolicyCurrent> {
        using is_local = std::true_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::PolicyCurrent;
        static ::omg::types::ref_type<::CORBA::PolicyCurrent> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::PolicyCurrent>(p); }
    };
    template <> struct traits<::CORBA::ExtAttributeDef> : detail::reference_traits<::CORBA::ExtAttributeDef> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ExtAttributeDef;
        static ::omg::types::ref_type<::CORBA::ExtAttributeDef> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ExtAttributeDef>(p); }
    };
    template <> struct traits<::CORBA::InterfaceAttrExtension> : detail::reference_traits<::CORBA::InterfaceAttrExtension> {
        using is_local = std::false_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::InterfaceAttrExtension;
        static ::omg::types::ref_type<::CORBA::InterfaceAttrExtension> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::InterfaceAttrExtension>(p); }
    };
    template <> struct traits<::CORBA::NVList> : detail::reference_traits<::CORBA::NVList> {
        using is_local = std::true_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::NVList;
        static ::omg::types::ref_type<::CORBA::NVList> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::NVList>(p); }
    };
    template <> struct traits<::CORBA::Context> : detail::reference_traits<::CORBA::Context> {
        using is_local = std::true_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::Context;
        static ::omg::types::ref_type<::CORBA::Context> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::Context>(p); }
    };
    template <> struct traits<::CORBA::Request> : detail::reference_traits<::CORBA::Request> {
        using is_local = std::true_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::Request;
        static ::omg::types::ref_type<::CORBA::Request> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::Request>(p); }
    };
    template <> struct traits<::CORBA::ORB> : detail::reference_traits<::CORBA::ORB> {
        using is_local = std::true_type;
        using is_abstract = std::false_type;
        using base_type = ::CORBA::ORB;
        static ::omg::types::ref_type<::CORBA::ORB> narrow(::omg::types::ref_type<::CORBA::Object> p) { return std::dynamic_pointer_cast<::CORBA::ORB>(p); }
    };
} // namespace CORBA

#endif // ZERODDS_IDL4_CORBA_MODULE_HPP
