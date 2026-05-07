// Omg.Types runtime — minimal foundation for ZeroDDS IDL4-CSharp
// codegen output (C5.3-b).
//
// This file is shipped alongside generated sources. It declares:
//   * Sequence-Contract:    ISequence<T>, IBoundedSequence<T>, BoundedList<T>
//   * Annotation-Bridge:    [Key], [Id], [Optional], [MustUnderstand],
//                           [External], [Nested], [Extensibility]
//   * Topic-Marker:         ITopicType<T>
//
// Target:  C# 12.0 / .NET 8+
// Source:  OMG IDL4-CSharp Mapping v1.0, formal/2024-12-01.
//          - Sequences:           §7.4 + Annex
//          - Annotations:         §6.7, mapped from XTypes §7.3.1.2
//          - Topic-Marker:        ZeroDDS-internal, opens FFI extension point

#nullable enable

using System;
using System.Collections;
using System.Collections.Generic;

namespace Omg.Types
{
    // -----------------------------------------------------------------------
    // Sequence-Contract (§7.4)
    // -----------------------------------------------------------------------

    /// <summary>
    /// IDL <c>sequence&lt;T&gt;</c> as C# contract: an ordered, mutable list
    /// of <typeparamref name="T"/>. Spec §7.4: unbounded sequences map to
    /// <c>ISequence&lt;T&gt;</c>; the underlying List is allowed to grow
    /// without limits.
    /// </summary>
    public interface ISequence<T> : IList<T>
    {
    }

    /// <summary>
    /// IDL <c>sequence&lt;T,N&gt;</c> as C# contract: an
    /// <see cref="ISequence{T}"/> with an immutable upper bound exposed
    /// through <see cref="Bound"/>. Implementations MUST refuse to grow
    /// past <c>Bound</c>.
    /// </summary>
    public interface IBoundedSequence<T> : ISequence<T>
    {
        /// <summary>The IDL bound (maximum element count).</summary>
        int Bound { get; }
    }

    /// <summary>
    /// Default mutable bounded-sequence container, backed by
    /// <see cref="List{T}"/>. <see cref="Add"/> and <see cref="Insert"/>
    /// throw <see cref="ArgumentOutOfRangeException"/> when the bound is
    /// exceeded. The setter on <see cref="this[int]"/> only validates
    /// the existing index, it cannot grow the list.
    /// </summary>
    public sealed class BoundedList<T> : IBoundedSequence<T>
    {
        private readonly List<T> _items;

        /// <summary>Construct an empty bounded list with the given bound.</summary>
        /// <exception cref="ArgumentOutOfRangeException">If
        /// <paramref name="bound"/> is negative.</exception>
        public BoundedList(int bound)
        {
            if (bound < 0)
            {
                throw new ArgumentOutOfRangeException(nameof(bound));
            }
            Bound = bound;
            _items = new List<T>(bound);
        }

        /// <summary>Construct a bounded list pre-filled with
        /// <paramref name="initial"/>; throws when the input already
        /// exceeds <paramref name="bound"/>.</summary>
        public BoundedList(int bound, IEnumerable<T> initial) : this(bound)
        {
            if (initial is null) throw new ArgumentNullException(nameof(initial));
            foreach (var x in initial) Add(x);
        }

        /// <inheritdoc />
        public int Bound { get; }

        /// <inheritdoc />
        public int Count => _items.Count;

        /// <inheritdoc />
        public bool IsReadOnly => false;

        /// <inheritdoc />
        public T this[int index]
        {
            get => _items[index];
            set => _items[index] = value;
        }

        /// <inheritdoc />
        public void Add(T item)
        {
            if (_items.Count >= Bound)
            {
                throw new ArgumentOutOfRangeException(
                    nameof(item),
                    $"BoundedList: bound {Bound} exceeded");
            }
            _items.Add(item);
        }

        /// <inheritdoc />
        public void Insert(int index, T item)
        {
            if (_items.Count >= Bound)
            {
                throw new ArgumentOutOfRangeException(
                    nameof(item),
                    $"BoundedList: bound {Bound} exceeded");
            }
            _items.Insert(index, item);
        }

        /// <inheritdoc />
        public void Clear() => _items.Clear();

        /// <inheritdoc />
        public bool Contains(T item) => _items.Contains(item);

        /// <inheritdoc />
        public void CopyTo(T[] array, int arrayIndex)
            => _items.CopyTo(array, arrayIndex);

        /// <inheritdoc />
        public IEnumerator<T> GetEnumerator() => _items.GetEnumerator();

        /// <inheritdoc />
        public int IndexOf(T item) => _items.IndexOf(item);

        /// <inheritdoc />
        public bool Remove(T item) => _items.Remove(item);

        /// <inheritdoc />
        public void RemoveAt(int index) => _items.RemoveAt(index);

        IEnumerator IEnumerable.GetEnumerator() => _items.GetEnumerator();
    }

    /// <summary>
    /// Default mutable unbounded-sequence container, backed by
    /// <see cref="List{T}"/>. Pure pass-through — included so generated
    /// code has a concrete type to instantiate when it needs a default
    /// value for an <see cref="ISequence{T}"/> field.
    /// </summary>
    public sealed class SequenceList<T> : ISequence<T>
    {
        private readonly List<T> _items = new();

        /// <inheritdoc />
        public int Count => _items.Count;

        /// <inheritdoc />
        public bool IsReadOnly => false;

        /// <inheritdoc />
        public T this[int index]
        {
            get => _items[index];
            set => _items[index] = value;
        }

        /// <inheritdoc />
        public void Add(T item) => _items.Add(item);

        /// <inheritdoc />
        public void Insert(int index, T item) => _items.Insert(index, item);

        /// <inheritdoc />
        public void Clear() => _items.Clear();

        /// <inheritdoc />
        public bool Contains(T item) => _items.Contains(item);

        /// <inheritdoc />
        public void CopyTo(T[] array, int arrayIndex)
            => _items.CopyTo(array, arrayIndex);

        /// <inheritdoc />
        public IEnumerator<T> GetEnumerator() => _items.GetEnumerator();

        /// <inheritdoc />
        public int IndexOf(T item) => _items.IndexOf(item);

        /// <inheritdoc />
        public bool Remove(T item) => _items.Remove(item);

        /// <inheritdoc />
        public void RemoveAt(int index) => _items.RemoveAt(index);

        IEnumerator IEnumerable.GetEnumerator() => _items.GetEnumerator();
    }

    // -----------------------------------------------------------------------
    // Topic-Marker (ZeroDDS-internal)
    // -----------------------------------------------------------------------

    /// <summary>
    /// Marker interface implemented by every generated top-level
    /// (i.e. non-<c>@nested</c>) struct. Phase 6 wires this into
    /// the <c>dds-cs</c> crate so the DDS layer can constrain
    /// <c>DataReader&lt;T&gt;</c>/<c>DataWriter&lt;T&gt;</c> to topic-able
    /// types only. Empty in C5.3-b — extension points come later.
    /// </summary>
    public interface ITopicType<T>
    {
    }

    // -----------------------------------------------------------------------
    // Annotation-Bridge (§6.7)
    // -----------------------------------------------------------------------

    /// <summary>Maps IDL <c>@key</c> onto generated members.</summary>
    [AttributeUsage(
        AttributeTargets.Property | AttributeTargets.Field,
        AllowMultiple = false)]
    public sealed class KeyAttribute : Attribute
    {
    }

    /// <summary>Maps IDL <c>@id(n)</c> onto generated members.</summary>
    [AttributeUsage(
        AttributeTargets.Property | AttributeTargets.Field,
        AllowMultiple = false)]
    public sealed class IdAttribute : Attribute
    {
        /// <summary>The Member-ID assigned by the IDL author.</summary>
        public int Value { get; }

        /// <summary>Construct an Id attribute carrying <paramref name="value"/>.</summary>
        public IdAttribute(int value)
        {
            Value = value;
        }
    }

    /// <summary>Maps IDL <c>@optional</c> onto generated members.
    /// The generated property type is also nullable (<c>T?</c>).</summary>
    [AttributeUsage(
        AttributeTargets.Property | AttributeTargets.Field,
        AllowMultiple = false)]
    public sealed class OptionalAttribute : Attribute
    {
    }

    /// <summary>Maps IDL <c>@must_understand</c> onto generated members.</summary>
    [AttributeUsage(
        AttributeTargets.Property | AttributeTargets.Field,
        AllowMultiple = false)]
    public sealed class MustUnderstandAttribute : Attribute
    {
    }

    /// <summary>Maps IDL <c>@external</c> onto generated members.</summary>
    [AttributeUsage(
        AttributeTargets.Property | AttributeTargets.Field,
        AllowMultiple = false)]
    public sealed class ExternalAttribute : Attribute
    {
    }

    /// <summary>Maps IDL <c>@nested</c> onto generated types
    /// (struct / union / typedef / exception).</summary>
    [AttributeUsage(
        AttributeTargets.Class | AttributeTargets.Struct,
        AllowMultiple = false,
        Inherited = false)]
    public sealed class NestedAttribute : Attribute
    {
    }

    /// <summary>Spec-defined extensibility kinds (XTypes §7.2.2.4).</summary>
    public enum ExtensibilityKind
    {
        /// <summary>FINAL — strict equality.</summary>
        Final,
        /// <summary>APPENDABLE — prefix match.</summary>
        Appendable,
        /// <summary>MUTABLE — id-based match.</summary>
        Mutable,
    }

    /// <summary>Maps IDL <c>@extensibility(K)</c> / <c>@final</c> /
    /// <c>@appendable</c> / <c>@mutable</c> onto generated types.</summary>
    [AttributeUsage(
        AttributeTargets.Class | AttributeTargets.Struct,
        AllowMultiple = false,
        Inherited = false)]
    public sealed class ExtensibilityAttribute : Attribute
    {
        /// <summary>The kind picked from the IDL annotation.</summary>
        public ExtensibilityKind Kind { get; }

        /// <summary>Construct an Extensibility attribute carrying
        /// <paramref name="kind"/>.</summary>
        public ExtensibilityAttribute(ExtensibilityKind kind)
        {
            Kind = kind;
        }
    }
}
