// The classes IDL4 to C# Language Mapping 1.0 Annex A.1 defines for the
// CORBA platform: Corba.UserException (A.1.1), Corba.TypeCode (A.1.2),
// Corba.IObjectOperations, Corba.IObject and Corba.Object (A.1.3) and
// Corba.Any (A.1.4).
//
// The rest of the Corba namespace is the C# of the CORBA module itself:
// `zerodds-idlc generate --lang csharp --csharp-mapping idl4
// --csharp-platform corba --csharp-corba-module` on an IDL file that
// includes `runtime/corba/CORBA_PIDL.idl` writes it under the IDL naming
// scheme, as A.1.2 requires. These classes name its types (TCKind,
// StructMember, IInterfaceDef, IORB, …) and compile together with it.
//
// Target: C# 11 / .NET 8.

#nullable enable

using System;
using System.Collections.Generic;

namespace Corba
{
    /// <summary>
    /// The base of every IDL exception on the CORBA platform, A.1.1:
    /// "public class UserException : System.Exception { }".
    /// </summary>
    public class UserException : Exception
    {
    }

    /// <summary>
    /// The operations of the CORBA <c>Object</c> interface, CORBA 3.3 part 1
    /// §8.3, mapped as §7.5 maps an interface (A.1.3), under the IDL naming
    /// scheme. <c>respository_id</c> in the clause's listing is
    /// <c>repository_id</c>, as §8.3.1.2 names it.
    /// </summary>
    public interface IObjectOperations
    {
        IInterfaceDef get_interface();

        bool is_nil();

        IObject duplicate();

        void release();

        bool is_a(string logical_type_id);

        bool non_existent();

        bool is_equivalent(IObject other_object);

        uint hash(uint maximum);

        void create_request(
            IContext ctx,
            string operation,
            INVList arg_list,
            ref NamedValue result,
            out IRequest req,
            uint req_flag);

        IPolicy get_policy(uint policy_type);

        Omg.Types.ISequence<IDomainManager> get_domain_managers();

        IObject set_policy_overrides(Omg.Types.ISequence<IPolicy> policies, SetOverrideType set_add);

        IPolicy get_client_policy(uint type);

        Omg.Types.ISequence<IPolicy> get_policy_overrides(Omg.Types.ISequence<uint> types);

        bool validate_connection(out Omg.Types.ISequence<IPolicy> inconsistent_policies);

        IObject get_component();

        string repository_id();

        IORB get_orb();
    }

    /// <summary>The CORBA <c>Object</c> interface, A.1.3.</summary>
    public interface IObject : IObjectOperations
    {
    }

    /// <summary>
    /// The class of the CORBA <c>Object</c> interface, A.1.3, which the class
    /// of every interface on the CORBA platform derives from (A.1.5). Its
    /// operations are the ORB's; this class leaves them abstract.
    /// </summary>
    public abstract class Object : IObject
    {
        public abstract IInterfaceDef get_interface();

        public abstract bool is_nil();

        public abstract IObject duplicate();

        public abstract void release();

        public abstract bool is_a(string logical_type_id);

        public abstract bool non_existent();

        public abstract bool is_equivalent(IObject other_object);

        public abstract uint hash(uint maximum);

        public abstract void create_request(
            IContext ctx,
            string operation,
            INVList arg_list,
            ref NamedValue result,
            out IRequest req,
            uint req_flag);

        public abstract IPolicy get_policy(uint policy_type);

        public abstract Omg.Types.ISequence<IDomainManager> get_domain_managers();

        public abstract IObject set_policy_overrides(
            Omg.Types.ISequence<IPolicy> policies, SetOverrideType set_add);

        public abstract IPolicy get_client_policy(uint type);

        public abstract Omg.Types.ISequence<IPolicy> get_policy_overrides(Omg.Types.ISequence<uint> types);

        public abstract bool validate_connection(out Omg.Types.ISequence<IPolicy> inconsistent_policies);

        public abstract IObject get_component();

        public abstract string repository_id();

        public abstract IORB get_orb();
    }

    /// <summary>
    /// The IDL <c>TypeCode</c> on the CORBA platform, A.1.2, with the
    /// behaviour of CORBA 3.3 part 1 §8.11.1. <c>Visibility</c> and
    /// <c>ValueModifier</c> are <c>typedef short</c> in the CORBA module and
    /// so <c>short</c> (§7.2.4.6). A type code is made by the static
    /// <c>create_*_tc</c> operations, the type code creation operations of
    /// the ORB (CORBA 3.3 part 1 §8.11.3) offered without an ORB, and by
    /// <see cref="get_primitive_tc"/>.
    /// </summary>
    public class TypeCode
    {
        /// <summary>Raised for an index at or beyond the number of members.</summary>
        public class Bounds : UserException
        {
        }

        /// <summary>Raised for an operation the kind does not have.</summary>
        public class BadKind : UserException
        {
        }

        private TCKind _kind;
        private string _id = "";
        private string _name = "";
        private string[] _memberNames = Array.Empty<string>();
        private TypeCode?[] _memberTypes = Array.Empty<TypeCode?>();
        private Any[] _memberLabels = Array.Empty<Any>();
        private short[] _memberVisibilities = Array.Empty<short>();
        private TypeCode? _discriminator;
        private int _defaultIndex = -1;
        private uint _length;
        private TypeCode? _content;
        private ushort _digits;
        private short _scale;
        private short _modifier;
        private TypeCode? _concreteBase;
        private string? _recursiveId;
        private TypeCode? _target;

        /// <summary>The type code of <c>tk_null</c>.</summary>
        public TypeCode()
        {
            _kind = TCKind.tk_null;
        }

        private TypeCode(TCKind kind)
        {
            _kind = kind;
        }

        public bool equal(TypeCode tc) => Compare(this, tc, false, new HashSet<(TypeCode, TypeCode)>(new PairComparer()));

        public bool equivalent(TypeCode tc) => Compare(this, tc, true, new HashSet<(TypeCode, TypeCode)>(new PairComparer()));

        public TypeCode get_compact_typecode() => Compact(this, new Dictionary<TypeCode, TypeCode>(ReferenceEqualityComparer.Instance))!;

        public TCKind kind() => Definition()._kind;

        public string id()
        {
            var d = Definition();
            d.Require(IdKinds);
            return d._id;
        }

        public string name()
        {
            var d = Definition();
            d.Require(IdKinds);
            return d._name;
        }

        public uint member_count()
        {
            var d = Definition();
            d.Require(MemberKinds);
            return (uint)d._memberNames.Length;
        }

        public string member_name(uint index)
        {
            var d = Definition();
            d.Require(MemberKinds);
            return d._memberNames[d.Within(index)];
        }

        public TypeCode member_type(uint index)
        {
            var d = Definition();
            d.Require(MemberTypeKinds);
            return d._memberTypes[d.Within(index)]!;
        }

        public Any member_label(uint index)
        {
            var d = Definition();
            d.Require(UnionKinds);
            return d._memberLabels[d.Within(index)];
        }

        public TypeCode discriminator_type()
        {
            var d = Definition();
            d.Require(UnionKinds);
            return d._discriminator!;
        }

        public int default_index()
        {
            var d = Definition();
            d.Require(UnionKinds);
            return d._defaultIndex;
        }

        public uint length()
        {
            var d = Definition();
            d.Require(LengthKinds);
            return d._length;
        }

        public TypeCode content_type()
        {
            var d = Definition();
            d.Require(ContentKinds);
            return d._content!;
        }

        public ushort fixed_digits()
        {
            var d = Definition();
            d.Require(FixedKinds);
            return d._digits;
        }

        public short fixed_scale()
        {
            var d = Definition();
            d.Require(FixedKinds);
            return d._scale;
        }

        public short member_visibility(uint index)
        {
            var d = Definition();
            d.Require(ValueKinds);
            return d._memberVisibilities[d.Within(index)];
        }

        public short type_modifier()
        {
            var d = Definition();
            d.Require(ValueKinds);
            return d._modifier;
        }

        /// <summary>The concrete base, <c>null</c> (a nil type code) for none.</summary>
        public TypeCode? concrete_base_type()
        {
            var d = Definition();
            d.Require(ValueKinds);
            return d._concreteBase;
        }

        // -- creating type codes, CORBA 3.3 part 1 §8.11.3 --------------------

        /// <summary>The type code of a kind without parameters; <c>tk_string</c>
        /// and <c>tk_wstring</c> unbounded, <c>tk_objref</c> for <c>Object</c>.</summary>
        public static TypeCode get_primitive_tc(TCKind kind)
        {
            switch (kind)
            {
                case TCKind.tk_null:
                case TCKind.tk_void:
                case TCKind.tk_short:
                case TCKind.tk_long:
                case TCKind.tk_ushort:
                case TCKind.tk_ulong:
                case TCKind.tk_float:
                case TCKind.tk_double:
                case TCKind.tk_boolean:
                case TCKind.tk_char:
                case TCKind.tk_octet:
                case TCKind.tk_any:
                case TCKind.tk_TypeCode:
                case TCKind.tk_Principal:
                case TCKind.tk_longlong:
                case TCKind.tk_ulonglong:
                case TCKind.tk_longdouble:
                case TCKind.tk_wchar:
                    return new TypeCode(kind);
                case TCKind.tk_string:
                    return create_string_tc(0);
                case TCKind.tk_wstring:
                    return create_wstring_tc(0);
                case TCKind.tk_objref:
                    return create_interface_tc("IDL:omg.org/CORBA/Object:1.0", "Object");
                default:
                    throw new BAD_PARAM(0, completion_status.COMPLETED_NO);
            }
        }

        public static TypeCode create_struct_tc(string id, string name, Omg.Types.ISequence<StructMember> members)
            => Aggregate(TCKind.tk_struct, id, name, members);

        public static TypeCode create_exception_tc(string id, string name, Omg.Types.ISequence<StructMember> members)
            => Aggregate(TCKind.tk_except, id, name, members);

        public static TypeCode create_union_tc(
            string id, string name, TypeCode discriminator_type, Omg.Types.ISequence<UnionMember> members)
        {
            var tc = Named(TCKind.tk_union, id, name);
            tc._discriminator = discriminator_type;
            int count = members.Count;
            tc._memberNames = new string[count];
            tc._memberTypes = new TypeCode?[count];
            tc._memberLabels = new Any[count];
            for (int i = 0; i < count; i++)
            {
                tc._memberNames[i] = members[i].name;
                tc._memberTypes[i] = members[i].type;
                tc._memberLabels[i] = members[i].label;
                // §8.11.1: "For the default member, the label is the zero octet."
                if (IsDefaultLabel(members[i].label))
                {
                    tc._defaultIndex = i;
                }
            }
            return BindRecursive(tc);
        }

        public static TypeCode create_enum_tc(string id, string name, Omg.Types.ISequence<string> members)
        {
            var tc = Named(TCKind.tk_enum, id, name);
            tc._memberNames = members.ToArray();
            return tc;
        }

        public static TypeCode create_alias_tc(string id, string name, TypeCode original_type)
        {
            var tc = Named(TCKind.tk_alias, id, name);
            tc._content = original_type;
            return tc;
        }

        public static TypeCode create_interface_tc(string id, string name) => Named(TCKind.tk_objref, id, name);

        public static TypeCode create_string_tc(uint bound) => new TypeCode(TCKind.tk_string) { _length = bound };

        public static TypeCode create_wstring_tc(uint bound) => new TypeCode(TCKind.tk_wstring) { _length = bound };

        public static TypeCode create_fixed_tc(ushort digits, short scale)
            => new TypeCode(TCKind.tk_fixed) { _digits = digits, _scale = scale };

        public static TypeCode create_sequence_tc(uint bound, TypeCode element_type)
            => new TypeCode(TCKind.tk_sequence) { _length = bound, _content = element_type };

        public static TypeCode create_array_tc(uint length, TypeCode element_type)
            => new TypeCode(TCKind.tk_array) { _length = length, _content = element_type };

        public static TypeCode create_value_tc(
            string id, string name, short type_modifier, TypeCode? concrete_base, Omg.Types.ISequence<ValueMember> members)
            => Value(TCKind.tk_value, id, name, type_modifier, concrete_base, members);

        public static TypeCode create_event_tc(
            string id, string name, short type_modifier, TypeCode? concrete_base, Omg.Types.ISequence<ValueMember> members)
            => Value(TCKind.tk_event, id, name, type_modifier, concrete_base, members);

        public static TypeCode create_value_box_tc(string id, string name, TypeCode boxed_type)
        {
            var tc = Named(TCKind.tk_value_box, id, name);
            tc._content = boxed_type;
            return BindRecursive(tc);
        }

        public static TypeCode create_native_tc(string id, string name) => Named(TCKind.tk_native, id, name);

        public static TypeCode create_abstract_interface_tc(string id, string name)
            => Named(TCKind.tk_abstract_interface, id, name);

        public static TypeCode create_local_interface_tc(string id, string name)
            => Named(TCKind.tk_local_interface, id, name);

        public static TypeCode create_component_tc(string id, string name) => Named(TCKind.tk_component, id, name);

        public static TypeCode create_home_tc(string id, string name) => Named(TCKind.tk_home, id, name);

        /// <summary>A placeholder for the type code with repository id
        /// <paramref name="id"/> that contains it; creating that type code
        /// binds it. Until then every operation raises BAD_TYPECODE.</summary>
        public static TypeCode create_recursive_tc(string id) => new TypeCode { _recursiveId = id };

        // -- the rest -----------------------------------------------------------

        private static readonly HashSet<TCKind> IdKinds = new()
        {
            TCKind.tk_objref, TCKind.tk_struct, TCKind.tk_union, TCKind.tk_enum, TCKind.tk_alias,
            TCKind.tk_value, TCKind.tk_value_box, TCKind.tk_native, TCKind.tk_abstract_interface,
            TCKind.tk_local_interface, TCKind.tk_except, TCKind.tk_component, TCKind.tk_home, TCKind.tk_event,
        };

        private static readonly HashSet<TCKind> MemberKinds = new()
        {
            TCKind.tk_struct, TCKind.tk_union, TCKind.tk_enum, TCKind.tk_value, TCKind.tk_event, TCKind.tk_except,
        };

        private static readonly HashSet<TCKind> MemberTypeKinds = new()
        {
            TCKind.tk_struct, TCKind.tk_union, TCKind.tk_value, TCKind.tk_event, TCKind.tk_except,
        };

        private static readonly HashSet<TCKind> UnionKinds = new() { TCKind.tk_union };

        private static readonly HashSet<TCKind> LengthKinds = new()
        {
            TCKind.tk_string, TCKind.tk_wstring, TCKind.tk_sequence, TCKind.tk_array,
        };

        private static readonly HashSet<TCKind> ContentKinds = new()
        {
            TCKind.tk_sequence, TCKind.tk_array, TCKind.tk_value_box, TCKind.tk_alias,
        };

        private static readonly HashSet<TCKind> FixedKinds = new() { TCKind.tk_fixed };

        private static readonly HashSet<TCKind> ValueKinds = new() { TCKind.tk_value, TCKind.tk_event };

        private TypeCode Definition()
        {
            var tc = this;
            while (tc._recursiveId != null)
            {
                tc = tc._target ?? throw new BAD_TYPECODE(0, completion_status.COMPLETED_NO);
            }
            return tc;
        }

        private void Require(HashSet<TCKind> kinds)
        {
            if (!kinds.Contains(_kind))
            {
                throw new BadKind();
            }
        }

        private int Within(uint index)
        {
            if (index >= (uint)_memberNames.Length)
            {
                throw new Bounds();
            }
            return (int)index;
        }

        private static TypeCode Named(TCKind kind, string? id, string? name)
            => new TypeCode(kind) { _id = id ?? "", _name = name ?? "" };

        private static TypeCode Aggregate(TCKind kind, string id, string name, Omg.Types.ISequence<StructMember> members)
        {
            var tc = Named(kind, id, name);
            int count = members.Count;
            tc._memberNames = new string[count];
            tc._memberTypes = new TypeCode?[count];
            for (int i = 0; i < count; i++)
            {
                tc._memberNames[i] = members[i].name;
                tc._memberTypes[i] = members[i].type;
            }
            return BindRecursive(tc);
        }

        private static TypeCode Value(
            TCKind kind, string id, string name, short modifier, TypeCode? concreteBase, Omg.Types.ISequence<ValueMember> members)
        {
            var tc = Named(kind, id, name);
            tc._modifier = modifier;
            tc._concreteBase = concreteBase;
            int count = members.Count;
            tc._memberNames = new string[count];
            tc._memberTypes = new TypeCode?[count];
            tc._memberVisibilities = new short[count];
            for (int i = 0; i < count; i++)
            {
                tc._memberNames[i] = members[i].name;
                tc._memberTypes[i] = members[i].type;
                tc._memberVisibilities[i] = members[i].access;
            }
            return BindRecursive(tc);
        }

        private static bool IsDefaultLabel(Any? label)
            => label != null && label.type.kind() == TCKind.tk_octet && label.extract_octet() == 0;

        private static TypeCode BindRecursive(TypeCode tc)
        {
            if (tc._id.Length > 0)
            {
                Bind(tc, tc._id, tc, new HashSet<TypeCode>(ReferenceEqualityComparer.Instance));
            }
            return tc;
        }

        private static void Bind(TypeCode? at, string id, TypeCode to, HashSet<TypeCode> seen)
        {
            if (at == null || !seen.Add(at))
            {
                return;
            }
            if (at._recursiveId != null)
            {
                if (at._target == null && at._recursiveId == id)
                {
                    at._target = to;
                }
                return;
            }
            foreach (var member in at._memberTypes)
            {
                Bind(member, id, to, seen);
            }
            Bind(at._content, id, to, seen);
            Bind(at._discriminator, id, to, seen);
            Bind(at._concreteBase, id, to, seen);
        }

        private sealed class PairComparer : IEqualityComparer<(TypeCode, TypeCode)>
        {
            public bool Equals((TypeCode, TypeCode) x, (TypeCode, TypeCode) y)
                => ReferenceEquals(x.Item1, y.Item1) && ReferenceEquals(x.Item2, y.Item2);

            public int GetHashCode((TypeCode, TypeCode) pair)
                => HashCode.Combine(
                    System.Runtime.CompilerServices.RuntimeHelpers.GetHashCode(pair.Item1),
                    System.Runtime.CompilerServices.RuntimeHelpers.GetHashCode(pair.Item2));
        }

        private static bool Compare(TypeCode? a, TypeCode? b, bool equivalence, HashSet<(TypeCode, TypeCode)> open)
        {
            if (a == null || b == null)
            {
                return ReferenceEquals(a, b);
            }
            a = a.Definition();
            b = b.Definition();
            if (equivalence)
            {
                // "recursively replace the TypeCode with the result of calling
                // content_type, until the kind is no longer tk_alias"
                while (a._kind == TCKind.tk_alias)
                {
                    a = a._content!.Definition();
                }
                while (b._kind == TCKind.tk_alias)
                {
                    b = b._content!.Definition();
                }
            }
            if (ReferenceEquals(a, b) || !open.Add((a, b)))
            {
                return true;
            }
            if (a._kind != b._kind)
            {
                return false;
            }
            bool hasId = IdKinds.Contains(a._kind);
            if (equivalence && hasId && a._id.Length > 0 && b._id.Length > 0)
            {
                return a._id == b._id;
            }
            if (!equivalence && hasId && (a._id != b._id || a._name != b._name))
            {
                return false;
            }
            if (MemberKinds.Contains(a._kind))
            {
                if (a._memberNames.Length != b._memberNames.Length)
                {
                    return false;
                }
                if (!equivalence)
                {
                    for (int i = 0; i < a._memberNames.Length; i++)
                    {
                        if (a._memberNames[i] != b._memberNames[i])
                        {
                            return false;
                        }
                    }
                }
            }
            if (MemberTypeKinds.Contains(a._kind))
            {
                for (int i = 0; i < a._memberTypes.Length; i++)
                {
                    if (!Compare(a._memberTypes[i], b._memberTypes[i], equivalence, open))
                    {
                        return false;
                    }
                }
            }
            if (UnionKinds.Contains(a._kind))
            {
                if (a._defaultIndex != b._defaultIndex
                    || !Compare(a._discriminator, b._discriminator, equivalence, open))
                {
                    return false;
                }
                for (int i = 0; i < a._memberLabels.Length; i++)
                {
                    if (!a._memberLabels[i].equal(b._memberLabels[i]))
                    {
                        return false;
                    }
                }
            }
            if (LengthKinds.Contains(a._kind) && a._length != b._length)
            {
                return false;
            }
            if (ContentKinds.Contains(a._kind) && !Compare(a._content, b._content, equivalence, open))
            {
                return false;
            }
            if (FixedKinds.Contains(a._kind) && (a._digits != b._digits || a._scale != b._scale))
            {
                return false;
            }
            if (ValueKinds.Contains(a._kind))
            {
                if (a._modifier != b._modifier
                    || a._memberVisibilities.Length != b._memberVisibilities.Length
                    || !Compare(a._concreteBase, b._concreteBase, equivalence, open))
                {
                    return false;
                }
                for (int i = 0; i < a._memberVisibilities.Length; i++)
                {
                    if (a._memberVisibilities[i] != b._memberVisibilities[i])
                    {
                        return false;
                    }
                }
            }
            return true;
        }

        private static TypeCode? Compact(TypeCode? tc, Dictionary<TypeCode, TypeCode> done)
        {
            if (tc == null)
            {
                return null;
            }
            if (done.TryGetValue(tc, out var known))
            {
                return known;
            }
            var output = new TypeCode(tc._kind);
            done[tc] = output;
            if (tc._recursiveId != null)
            {
                output._recursiveId = tc._recursiveId;
                output._target = Compact(tc._target, done);
                return output;
            }
            output._id = tc._id;
            // "strips out all optional name and member name fields"
            output._name = "";
            output._memberNames = new string[tc._memberNames.Length];
            Array.Fill(output._memberNames, "");
            output._memberTypes = new TypeCode?[tc._memberTypes.Length];
            for (int i = 0; i < tc._memberTypes.Length; i++)
            {
                output._memberTypes[i] = Compact(tc._memberTypes[i], done);
            }
            output._memberLabels = (Any[])tc._memberLabels.Clone();
            output._memberVisibilities = (short[])tc._memberVisibilities.Clone();
            output._discriminator = Compact(tc._discriminator, done);
            output._defaultIndex = tc._defaultIndex;
            output._length = tc._length;
            output._content = Compact(tc._content, done);
            output._digits = tc._digits;
            output._scale = tc._scale;
            output._modifier = tc._modifier;
            output._concreteBase = Compact(tc._concreteBase, done);
            return output;
        }
    }

    /// <summary>
    /// The IDL <c>any</c> on the CORBA platform, A.1.4. An
    /// <c>insert_&lt;type&gt;</c> sets the type code and the value; an
    /// <c>extract_&lt;type&gt;</c> returns the value when the type code held
    /// is equivalent to that of <c>&lt;type&gt;</c> and raises
    /// <c>BAD_OPERATION</c> otherwise.
    /// </summary>
    public class Any
    {
        private object? _value;
        private bool _holds;

        /// <summary>An any of type <c>tk_null</c>.</summary>
        public Any()
        {
            type = new TypeCode();
        }

        /// <summary>A copy of <paramref name="other"/> (its value shared).</summary>
        public Any(Any other)
        {
            if (other is null) throw new ArgumentNullException(nameof(other));
            type = other.type;
            _value = other._value;
            _holds = other._holds;
        }

        /// <summary>The type code of the value held.</summary>
        public TypeCode type { get; private set; }

        /// <summary>Whether both hold equal type codes and equal values.</summary>
        public bool equal(Any a)
        {
            if (a is null || !type.equal(a.type) || _holds != a._holds)
            {
                return false;
            }
            if (_value is Any inner && a._value is Any other)
            {
                return inner.equal(other);
            }
            return Equals(_value, a._value);
        }

        public void insert_short(short value) => Hold(TCKind.tk_short, value);

        public short extract_short() => (short)Held(TCKind.tk_short);

        public void insert_long(int value) => Hold(TCKind.tk_long, value);

        public int extract_long() => (int)Held(TCKind.tk_long);

        public void insert_longlong(long value) => Hold(TCKind.tk_longlong, value);

        public long extract_longlong() => (long)Held(TCKind.tk_longlong);

        public void insert_ushort(ushort value) => Hold(TCKind.tk_ushort, value);

        public ushort extract_ushort() => (ushort)Held(TCKind.tk_ushort);

        public void insert_ulong(uint value) => Hold(TCKind.tk_ulong, value);

        public uint extract_ulong() => (uint)Held(TCKind.tk_ulong);

        public void insert_ulonglong(ulong value) => Hold(TCKind.tk_ulonglong, value);

        public ulong extract_ulonglong() => (ulong)Held(TCKind.tk_ulonglong);

        public void insert_float(float value) => Hold(TCKind.tk_float, value);

        public float extract_float() => (float)Held(TCKind.tk_float);

        public void insert_double(double value) => Hold(TCKind.tk_double, value);

        public double extract_double() => (double)Held(TCKind.tk_double);

        public void insert_boolean(bool value) => Hold(TCKind.tk_boolean, value);

        public bool extract_boolean() => (bool)Held(TCKind.tk_boolean);

        public void insert_char(char value) => Hold(TCKind.tk_char, value);

        public char extract_char() => (char)Held(TCKind.tk_char);

        public void insert_wchar(char value) => Hold(TCKind.tk_wchar, value);

        public char extract_wchar() => (char)Held(TCKind.tk_wchar);

        public void insert_octet(byte value) => Hold(TCKind.tk_octet, value);

        public byte extract_octet() => (byte)Held(TCKind.tk_octet);

        public void insert_any(Any value) => Hold(TCKind.tk_any, value);

        public Any extract_any() => (Any)Held(TCKind.tk_any);

        public void insert_object(Object value) => Hold(TCKind.tk_objref, value);

        public Object extract_object() => (Object)Held(TCKind.tk_objref);

        private void Hold(TCKind kind, object? value)
        {
            type = TypeCode.get_primitive_tc(kind);
            _value = value;
            _holds = true;
        }

        private object Held(TCKind kind)
        {
            if (!_holds || !type.equivalent(TypeCode.get_primitive_tc(kind)))
            {
                throw new BAD_OPERATION(0, completion_status.COMPLETED_NO);
            }
            return _value!;
        }
    }
}
