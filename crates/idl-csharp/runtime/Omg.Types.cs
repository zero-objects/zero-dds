// Omg.Types: the runtime the IDL4 to C# Language Mapping 1.0 names
// (OMG formal/2021-07-01), and the annotation bridge of the XCDR2 C#
// binding.
//
// It declares:
//   * ISequence<T> with the methods of §7.2.4.2.1, IBoundedSequence<T>,
//     their implementations SequenceList<T> and BoundedList<T>, and
//     BoundedDictionary<TKey, TValue> for a bounded map (§7.14.3.1);
//   * Any (§7.3);
//   * FactoryAttribute (§7.6), OptionalAttribute (Table 7.5),
//     KeyAttribute (Table 7.6), UnitAttribute (Table 7.7);
//   * the helpers generated code calls: Structural (IEquatable<T> over
//     collections), Sequences, Arrays and Maps (deep copies), Fixed (the
//     range check of §7.2.4.2.4);
//   * the XCDR2 binding's annotation bridge and ITopicType<T>.
//
// Target: C# 11 / .NET 8.

#nullable enable

using System;
using System.Collections;
using System.Collections.Generic;
using System.Collections.ObjectModel;

namespace Omg.Types
{
    // -----------------------------------------------------------------------
    // Sequences, §7.2.4.2.1
    // -----------------------------------------------------------------------

    /// <summary>
    /// An IDL <c>sequence&lt;T&gt;</c>, §7.2.4.2.1: an
    /// <see cref="IList{T}"/> with every method of
    /// <see cref="List{T}"/>, the signatures and semantics of their namesakes
    /// there.
    /// </summary>
    public interface ISequence<T> : IList<T>
    {
        int Capacity { get; set; }
        void AddRange(IEnumerable<T> collection);
        ReadOnlyCollection<T> AsReadOnly();
        int BinarySearch(int index, int count, T item, IComparer<T>? comparer);
        int BinarySearch(T item);
        int BinarySearch(T item, IComparer<T>? comparer);
        ISequence<TOutput> ConvertAll<TOutput>(Converter<T, TOutput> converter);
        void CopyTo(T[] array);
        void CopyTo(int index, T[] array, int arrayIndex, int count);
        bool Exists(Predicate<T> match);
        T? Find(Predicate<T> match);
        ISequence<T> FindAll(Predicate<T> match);
        int FindIndex(Predicate<T> match);
        int FindIndex(int startIndex, Predicate<T> match);
        int FindIndex(int startIndex, int count, Predicate<T> match);
        T? FindLast(Predicate<T> match);
        int FindLastIndex(Predicate<T> match);
        int FindLastIndex(int startIndex, Predicate<T> match);
        int FindLastIndex(int startIndex, int count, Predicate<T> match);
        void ForEach(Action<T> action);
        int IndexOf(T item, int index);
        int IndexOf(T item, int index, int count);
        void InsertRange(int index, IEnumerable<T> collection);
        int LastIndexOf(T item);
        int LastIndexOf(T item, int index);
        int LastIndexOf(T item, int index, int count);
        int RemoveAll(Predicate<T> match);
        void RemoveRange(int index, int count);
        void Reverse();
        void Reverse(int index, int count);
        void Sort();
        void Sort(IComparer<T>? comparer);
        void Sort(int index, int count, IComparer<T>? comparer);
        T[] ToArray();
        void TrimExcess();
        bool TrueForAll(Predicate<T> match);
    }

    /// <summary>
    /// An IDL <c>sequence&lt;T, N&gt;</c>: an <see cref="ISequence{T}"/>
    /// that refuses to grow past <see cref="Bound"/> ("Bounds checking on
    /// bounded sequences may raise an exception if necessary").
    /// </summary>
    public interface IBoundedSequence<T> : ISequence<T>
    {
        /// <summary>The IDL bound.</summary>
        int Bound { get; }
    }

    /// <summary>
    /// The sequence behind <see cref="SequenceList{T}"/> and
    /// <see cref="BoundedList{T}"/>: a <see cref="List{T}"/>, and a check
    /// before every operation that adds elements.
    /// </summary>
    public abstract class SequenceBase<T> : ISequence<T>
    {
        private readonly List<T> _items;

        /// <summary>An empty sequence.</summary>
        protected SequenceBase()
        {
            _items = new List<T>();
        }

        /// <summary>Throws when <paramref name="added"/> more elements would
        /// not fit.</summary>
        protected abstract void CheckRoom(int added);

        /// <summary>An empty sequence of the same kind and bound.</summary>
        protected abstract ISequence<TOutput> Empty<TOutput>();

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
        public int Capacity
        {
            get => _items.Capacity;
            set => _items.Capacity = value;
        }

        /// <inheritdoc />
        public void Add(T item)
        {
            CheckRoom(1);
            _items.Add(item);
        }

        /// <inheritdoc />
        public void AddRange(IEnumerable<T> collection)
        {
            var list = new List<T>(collection ?? throw new ArgumentNullException(nameof(collection)));
            CheckRoom(list.Count);
            _items.AddRange(list);
        }

        /// <inheritdoc />
        public ReadOnlyCollection<T> AsReadOnly() => _items.AsReadOnly();

        /// <inheritdoc />
        public int BinarySearch(int index, int count, T item, IComparer<T>? comparer)
            => _items.BinarySearch(index, count, item, comparer);

        /// <inheritdoc />
        public int BinarySearch(T item) => _items.BinarySearch(item);

        /// <inheritdoc />
        public int BinarySearch(T item, IComparer<T>? comparer) => _items.BinarySearch(item, comparer);

        /// <inheritdoc />
        public void Clear() => _items.Clear();

        /// <inheritdoc />
        public bool Contains(T item) => _items.Contains(item);

        /// <inheritdoc />
        public ISequence<TOutput> ConvertAll<TOutput>(Converter<T, TOutput> converter)
        {
            var output = Empty<TOutput>();
            output.AddRange(_items.ConvertAll(converter));
            return output;
        }

        /// <inheritdoc />
        public void CopyTo(T[] array) => _items.CopyTo(array);

        /// <inheritdoc />
        public void CopyTo(T[] array, int arrayIndex) => _items.CopyTo(array, arrayIndex);

        /// <inheritdoc />
        public void CopyTo(int index, T[] array, int arrayIndex, int count)
            => _items.CopyTo(index, array, arrayIndex, count);

        /// <inheritdoc />
        public bool Exists(Predicate<T> match) => _items.Exists(match);

        /// <inheritdoc />
        public T? Find(Predicate<T> match) => _items.Find(match);

        /// <inheritdoc />
        public ISequence<T> FindAll(Predicate<T> match)
        {
            var output = Empty<T>();
            output.AddRange(_items.FindAll(match));
            return output;
        }

        /// <inheritdoc />
        public int FindIndex(Predicate<T> match) => _items.FindIndex(match);

        /// <inheritdoc />
        public int FindIndex(int startIndex, Predicate<T> match) => _items.FindIndex(startIndex, match);

        /// <inheritdoc />
        public int FindIndex(int startIndex, int count, Predicate<T> match)
            => _items.FindIndex(startIndex, count, match);

        /// <inheritdoc />
        public T? FindLast(Predicate<T> match) => _items.FindLast(match);

        /// <inheritdoc />
        public int FindLastIndex(Predicate<T> match) => _items.FindLastIndex(match);

        /// <inheritdoc />
        public int FindLastIndex(int startIndex, Predicate<T> match)
            => _items.FindLastIndex(startIndex, match);

        /// <inheritdoc />
        public int FindLastIndex(int startIndex, int count, Predicate<T> match)
            => _items.FindLastIndex(startIndex, count, match);

        /// <inheritdoc />
        public void ForEach(Action<T> action) => _items.ForEach(action);

        /// <inheritdoc />
        public IEnumerator<T> GetEnumerator() => _items.GetEnumerator();

        IEnumerator IEnumerable.GetEnumerator() => _items.GetEnumerator();

        /// <inheritdoc />
        public int IndexOf(T item) => _items.IndexOf(item);

        /// <inheritdoc />
        public int IndexOf(T item, int index) => _items.IndexOf(item, index);

        /// <inheritdoc />
        public int IndexOf(T item, int index, int count) => _items.IndexOf(item, index, count);

        /// <inheritdoc />
        public void Insert(int index, T item)
        {
            CheckRoom(1);
            _items.Insert(index, item);
        }

        /// <inheritdoc />
        public void InsertRange(int index, IEnumerable<T> collection)
        {
            var list = new List<T>(collection ?? throw new ArgumentNullException(nameof(collection)));
            CheckRoom(list.Count);
            _items.InsertRange(index, list);
        }

        /// <inheritdoc />
        public int LastIndexOf(T item) => _items.LastIndexOf(item);

        /// <inheritdoc />
        public int LastIndexOf(T item, int index) => _items.LastIndexOf(item, index);

        /// <inheritdoc />
        public int LastIndexOf(T item, int index, int count) => _items.LastIndexOf(item, index, count);

        /// <inheritdoc />
        public bool Remove(T item) => _items.Remove(item);

        /// <inheritdoc />
        public int RemoveAll(Predicate<T> match) => _items.RemoveAll(match);

        /// <inheritdoc />
        public void RemoveAt(int index) => _items.RemoveAt(index);

        /// <inheritdoc />
        public void RemoveRange(int index, int count) => _items.RemoveRange(index, count);

        /// <inheritdoc />
        public void Reverse() => _items.Reverse();

        /// <inheritdoc />
        public void Reverse(int index, int count) => _items.Reverse(index, count);

        /// <inheritdoc />
        public void Sort() => _items.Sort();

        /// <inheritdoc />
        public void Sort(IComparer<T>? comparer) => _items.Sort(comparer);

        /// <inheritdoc />
        public void Sort(int index, int count, IComparer<T>? comparer) => _items.Sort(index, count, comparer);

        /// <inheritdoc />
        public T[] ToArray() => _items.ToArray();

        /// <inheritdoc />
        public void TrimExcess() => _items.TrimExcess();

        /// <inheritdoc />
        public bool TrueForAll(Predicate<T> match) => _items.TrueForAll(match);
    }

    /// <summary>An unbounded IDL sequence.</summary>
    public sealed class SequenceList<T> : SequenceBase<T>
    {
        /// <summary>An empty sequence.</summary>
        public SequenceList()
        {
        }

        /// <summary>A sequence holding <paramref name="initial"/>.</summary>
        public SequenceList(IEnumerable<T> initial)
        {
            AddRange(initial);
        }

        /// <inheritdoc />
        protected override void CheckRoom(int added)
        {
        }

        /// <inheritdoc />
        protected override ISequence<TOutput> Empty<TOutput>() => new SequenceList<TOutput>();
    }

    /// <summary>A bounded IDL sequence: adding past the bound throws
    /// <see cref="ArgumentOutOfRangeException"/>.</summary>
    public sealed class BoundedList<T> : SequenceBase<T>, IBoundedSequence<T>
    {
        /// <summary>An empty sequence of bound <paramref name="bound"/>.</summary>
        public BoundedList(int bound)
        {
            if (bound < 0)
            {
                throw new ArgumentOutOfRangeException(nameof(bound));
            }
            Bound = bound;
        }

        /// <summary>A sequence of bound <paramref name="bound"/> holding
        /// <paramref name="initial"/>.</summary>
        public BoundedList(int bound, IEnumerable<T> initial) : this(bound)
        {
            AddRange(initial);
        }

        /// <inheritdoc />
        public int Bound { get; }

        /// <inheritdoc />
        protected override void CheckRoom(int added)
        {
            if (Count + added > Bound)
            {
                throw new ArgumentOutOfRangeException(
                    nameof(added), $"{Count + added} elements, the bound is {Bound}");
            }
        }

        /// <inheritdoc />
        protected override ISequence<TOutput> Empty<TOutput>() => new BoundedList<TOutput>(Bound);
    }

    /// <summary>
    /// A bounded IDL <c>map&lt;K, V, N&gt;</c> (§7.14.3.1: "Bounds checking
    /// shall raise an exception if necessary"): adding past the bound throws
    /// <see cref="ArgumentOutOfRangeException"/>.
    /// </summary>
    public sealed class BoundedDictionary<TKey, TValue> : IDictionary<TKey, TValue>
        where TKey : notnull
    {
        private readonly Dictionary<TKey, TValue> _entries = new();

        /// <summary>An empty map of bound <paramref name="bound"/>.</summary>
        public BoundedDictionary(int bound)
        {
            if (bound < 0)
            {
                throw new ArgumentOutOfRangeException(nameof(bound));
            }
            Bound = bound;
        }

        /// <summary>The IDL bound.</summary>
        public int Bound { get; }

        private void CheckRoom(TKey key)
        {
            if (!_entries.ContainsKey(key) && _entries.Count >= Bound)
            {
                throw new ArgumentOutOfRangeException(
                    nameof(key), $"{_entries.Count + 1} entries, the bound is {Bound}");
            }
        }

        /// <inheritdoc />
        public TValue this[TKey key]
        {
            get => _entries[key];
            set
            {
                CheckRoom(key);
                _entries[key] = value;
            }
        }

        /// <inheritdoc />
        public ICollection<TKey> Keys => _entries.Keys;

        /// <inheritdoc />
        public ICollection<TValue> Values => _entries.Values;

        /// <inheritdoc />
        public int Count => _entries.Count;

        /// <inheritdoc />
        public bool IsReadOnly => false;

        /// <inheritdoc />
        public void Add(TKey key, TValue value)
        {
            CheckRoom(key);
            _entries.Add(key, value);
        }

        /// <inheritdoc />
        public void Add(KeyValuePair<TKey, TValue> item) => Add(item.Key, item.Value);

        /// <inheritdoc />
        public void Clear() => _entries.Clear();

        /// <inheritdoc />
        public bool Contains(KeyValuePair<TKey, TValue> item)
            => ((ICollection<KeyValuePair<TKey, TValue>>)_entries).Contains(item);

        /// <inheritdoc />
        public bool ContainsKey(TKey key) => _entries.ContainsKey(key);

        /// <inheritdoc />
        public void CopyTo(KeyValuePair<TKey, TValue>[] array, int arrayIndex)
            => ((ICollection<KeyValuePair<TKey, TValue>>)_entries).CopyTo(array, arrayIndex);

        /// <inheritdoc />
        public IEnumerator<KeyValuePair<TKey, TValue>> GetEnumerator() => _entries.GetEnumerator();

        IEnumerator IEnumerable.GetEnumerator() => _entries.GetEnumerator();

        /// <inheritdoc />
        public bool Remove(TKey key) => _entries.Remove(key);

        /// <inheritdoc />
        public bool Remove(KeyValuePair<TKey, TValue> item)
            => ((ICollection<KeyValuePair<TKey, TValue>>)_entries).Remove(item);

        /// <inheritdoc />
        public bool TryGetValue(TKey key, out TValue value)
        {
            if (_entries.TryGetValue(key, out var found))
            {
                value = found;
                return true;
            }
            value = default!;
            return false;
        }
    }

    // -----------------------------------------------------------------------
    // What generated code calls
    // -----------------------------------------------------------------------

    /// <summary>
    /// Structural equality and hash codes over the members of a generated
    /// type, collections element by element: "The class shall implement the
    /// IEquatable&lt;T&gt; interface" (§7.2.4.3.1) for a type holding
    /// sequences, arrays and maps.
    /// </summary>
    public static class Structural
    {
        /// <summary>Whether <paramref name="a"/> and <paramref name="b"/>
        /// hold the same values.</summary>
        public static bool Equal(object? a, object? b)
        {
            if (ReferenceEquals(a, b))
            {
                return true;
            }
            if (a is null || b is null)
            {
                return false;
            }
            if (a is BitArray bitsA && b is BitArray bitsB)
            {
                if (bitsA.Length != bitsB.Length)
                {
                    return false;
                }
                for (int i = 0; i < bitsA.Length; i++)
                {
                    if (bitsA[i] != bitsB[i])
                    {
                        return false;
                    }
                }
                return true;
            }
            var entriesA = Entries(a);
            var entriesB = Entries(b);
            if (entriesA != null && entriesB != null)
            {
                if (entriesA.Count != entriesB.Count)
                {
                    return false;
                }
                foreach (var (key, value) in entriesA)
                {
                    int match = entriesB.FindIndex(entry => Equal(entry.Key, key));
                    if (match < 0 || !Equal(value, entriesB[match].Value))
                    {
                        return false;
                    }
                }
                return true;
            }
            if (a is Array arrayA && b is Array arrayB)
            {
                if (arrayA.Rank != arrayB.Rank)
                {
                    return false;
                }
                for (int r = 0; r < arrayA.Rank; r++)
                {
                    if (arrayA.GetLength(r) != arrayB.GetLength(r))
                    {
                        return false;
                    }
                }
            }
            if (a is IEnumerable sequenceA && a is not string && b is IEnumerable sequenceB && b is not string)
            {
                var e1 = sequenceA.GetEnumerator();
                var e2 = sequenceB.GetEnumerator();
                while (true)
                {
                    bool more1 = e1.MoveNext();
                    bool more2 = e2.MoveNext();
                    if (more1 != more2)
                    {
                        return false;
                    }
                    if (!more1)
                    {
                        return true;
                    }
                    if (!Equal(e1.Current, e2.Current))
                    {
                        return false;
                    }
                }
            }
            return a.Equals(b);
        }

        /// <summary>A hash code agreeing with <see cref="Equal"/>.</summary>
        public static int Hash(object? value)
        {
            if (value is null)
            {
                return 0;
            }
            if (value is BitArray bits)
            {
                var hash = new HashCode();
                for (int i = 0; i < bits.Length; i++)
                {
                    hash.Add(bits[i]);
                }
                return hash.ToHashCode();
            }
            var entries = Entries(value);
            if (entries != null)
            {
                // Order-free: the sum of the entries' hashes.
                int sum = 0;
                foreach (var (key, entry) in entries)
                {
                    sum = unchecked(sum + HashCode.Combine(Hash(key), Hash(entry)));
                }
                return sum;
            }
            if (value is IEnumerable sequence && value is not string)
            {
                var hash = new HashCode();
                foreach (var element in sequence)
                {
                    hash.Add(Hash(element));
                }
                return hash.ToHashCode();
            }
            return value.GetHashCode();
        }

        /// <summary>The entries of a map, generic or not; <c>null</c> for
        /// anything else.</summary>
        private static List<KeyValuePair<object?, object?>>? Entries(object value)
        {
            if (value is IDictionary dictionary)
            {
                var output = new List<KeyValuePair<object?, object?>>();
                foreach (DictionaryEntry entry in dictionary)
                {
                    output.Add(new KeyValuePair<object?, object?>(entry.Key, entry.Value));
                }
                return output;
            }
            foreach (var implemented in value.GetType().GetInterfaces())
            {
                if (implemented.IsGenericType
                    && implemented.GetGenericTypeDefinition() == typeof(IDictionary<,>))
                {
                    var output = new List<KeyValuePair<object?, object?>>();
                    foreach (var entry in (IEnumerable)value)
                    {
                        var type = entry!.GetType();
                        output.Add(new KeyValuePair<object?, object?>(
                            type.GetProperty("Key")!.GetValue(entry),
                            type.GetProperty("Value")!.GetValue(entry)));
                    }
                    return output;
                }
            }
            return null;
        }
    }

    /// <summary>Deep copies of sequences: same kind, same bound, every
    /// element copied.</summary>
    public static class Sequences
    {
        /// <summary>A copy of <paramref name="source"/>, each element through
        /// <paramref name="copy"/>; <c>null</c> for <c>null</c>.</summary>
        public static ISequence<T>? Copy<T>(ISequence<T>? source, Func<T, T> copy)
        {
            if (source is null)
            {
                return null;
            }
            ISequence<T> output = source is IBoundedSequence<T> bounded
                ? new BoundedList<T>(bounded.Bound)
                : new SequenceList<T>();
            foreach (var element in source)
            {
                output.Add(copy(element));
            }
            return output;
        }

        /// <summary>A copy of an external sequence (Table 7.8), each element
        /// through <paramref name="copy"/>.</summary>
        public static IEnumerable<T>? Copy<T>(IEnumerable<T>? source, Func<T, T> copy)
        {
            if (source is null)
            {
                return null;
            }
            var output = new SequenceList<T>();
            foreach (var element in source)
            {
                output.Add(copy(element));
            }
            return output;
        }
    }

    /// <summary>Arrays of a declared size, filled and copied.</summary>
    public static class Arrays
    {
        /// <summary><paramref name="array"/> with every element set by
        /// <paramref name="create"/>, one call per element.</summary>
        public static TArray Fill<TArray, T>(TArray array, Func<T> create) where TArray : class
        {
            var target = (Array)(object)array;
            var index = new int[target.Rank];
            Visit(target, 0, index, (at) => target.SetValue(create(), at));
            return array;
        }

        /// <summary>A copy of <paramref name="source"/>, each element through
        /// <paramref name="copy"/>; <c>null</c> for <c>null</c>.</summary>
        public static TArray? Copy<TArray, T>(TArray? source, Func<T, T> copy) where TArray : class
        {
            if (source is null)
            {
                return null;
            }
            var from = (Array)(object)source;
            var output = (Array)from.Clone();
            var index = new int[from.Rank];
            Visit(from, 0, index, (at) => output.SetValue(copy((T)from.GetValue(at)!), at));
            return (TArray)(object)output;
        }

        private static void Visit(Array array, int rank, int[] index, Action<int[]> action)
        {
            if (rank == array.Rank)
            {
                action(index);
                return;
            }
            for (int i = 0; i < array.GetLength(rank); i++)
            {
                index[rank] = i;
                Visit(array, rank + 1, index, action);
            }
        }
    }

    /// <summary>Deep copies of maps: same kind, same bound, every key and
    /// value copied.</summary>
    public static class Maps
    {
        /// <summary>A copy of <paramref name="source"/>; <c>null</c> for
        /// <c>null</c>.</summary>
        public static IDictionary<TKey, TValue>? Copy<TKey, TValue>(
            IDictionary<TKey, TValue>? source, Func<TKey, TKey> copyKey, Func<TValue, TValue> copyValue)
            where TKey : notnull
        {
            if (source is null)
            {
                return null;
            }
            IDictionary<TKey, TValue> output = source is BoundedDictionary<TKey, TValue> bounded
                ? new BoundedDictionary<TKey, TValue>(bounded.Bound)
                : new Dictionary<TKey, TValue>();
            foreach (var entry in source)
            {
                output.Add(copyKey(entry.Key), copyValue(entry.Value));
            }
            return output;
        }
    }

    /// <summary>The range check of an IDL <c>fixed&lt;digits, scale&gt;</c>
    /// held as <c>decimal</c>, §7.2.4.2.4: "Range checking shall raise a
    /// System.ArithmeticException exception, or a derived exception, if
    /// necessary."</summary>
    public static class Fixed
    {
        /// <summary>Throws when <paramref name="value"/> has more digits after
        /// the point than <paramref name="scale"/>, or more before it than
        /// <paramref name="digits"/> minus <paramref name="scale"/>.</summary>
        public static void Check(decimal value, int digits, int scale, string what)
        {
            if (decimal.Round(value, scale) != value)
            {
                throw new ArithmeticException($"{what}: {value} has more than {scale} digits after the point");
            }
            decimal limit = 1m;
            for (int i = 0; i < digits - scale; i++)
            {
                limit *= 10m;
            }
            if (Math.Abs(decimal.Truncate(value)) >= limit)
            {
                throw new ArithmeticException(
                    $"{what}: {value} has more than {digits - scale} digits before the point");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Any, §7.3
    // -----------------------------------------------------------------------

    /// <summary>
    /// The IDL <c>any</c> (§7.3): "operations that allow programmers to
    /// insert and access the value contained in an any instance as well as
    /// the actual type of that value".
    /// </summary>
    public sealed class Any : IEquatable<Any>
    {
        /// <summary>The repository id of the value's IDL type, when known.</summary>
        public string? TypeId;

        /// <summary>The value.</summary>
        public object? Value;

        /// <summary>An any holding nothing.</summary>
        public Any()
        {
        }

        /// <summary>A copy of <paramref name="other"/> (its value shared).</summary>
        public Any(Any other)
        {
            if (other is null) throw new ArgumentNullException(nameof(other));
            TypeId = other.TypeId;
            Value = other.Value;
        }

        /// <summary>Holds <paramref name="value"/> from now on.</summary>
        public void Insert<T>(T value)
        {
            Value = value;
        }

        /// <summary>The value held as a <typeparamref name="T"/>.</summary>
        /// <exception cref="InvalidCastException">When it is no
        /// <typeparamref name="T"/>.</exception>
        public T Extract<T>()
        {
            if (Value is T value)
            {
                return value;
            }
            throw new InvalidCastException($"the any holds {Value?.GetType().ToString() ?? "nothing"}, not {typeof(T)}");
        }

        /// <summary>The C# type of the value held, <c>null</c> for none.</summary>
        public Type? Type => Value?.GetType();

        /// <inheritdoc />
        public bool Equals(Any? other) => other is not null && Structural.Equal(Value, other.Value);

        /// <inheritdoc />
        public override bool Equals(object? obj) => Equals(obj as Any);

        /// <inheritdoc />
        public override int GetHashCode() => Structural.Hash(Value);
    }

    // -----------------------------------------------------------------------
    // The attributes of §7.6 and Tables 7.5 to 7.7
    // -----------------------------------------------------------------------

    /// <summary>A value type's initializer, §7.6.</summary>
    public class FactoryAttribute : Attribute
    {
        /// <summary>Marks a factory.</summary>
        public FactoryAttribute()
        {
            IsFactory = true;
        }

        /// <summary>Whether it is one.</summary>
        public bool IsFactory { get; set; }
    }

    /// <summary>An <c>@optional</c> member or parameter, Table 7.5.</summary>
    [AttributeUsage(
        AttributeTargets.Property | AttributeTargets.Field | AttributeTargets.Parameter,
        AllowMultiple = false)]
    public class OptionalAttribute : Attribute
    {
        /// <summary>Marks an optional element.</summary>
        public OptionalAttribute()
        {
            IsOptional = true;
        }

        /// <summary>Whether it is one.</summary>
        public bool IsOptional { get; set; }
    }

    /// <summary>An <c>@key</c> member, Table 7.6.</summary>
    [AttributeUsage(AttributeTargets.Property | AttributeTargets.Field, AllowMultiple = false)]
    public class KeyAttribute : Attribute
    {
        /// <summary>Marks a key member.</summary>
        public KeyAttribute()
        {
            IsKey = true;
        }

        /// <summary>Whether it is one.</summary>
        public bool IsKey { get; set; }
    }

    /// <summary>The <c>@unit</c> of an element, Table 7.7.</summary>
    public class UnitAttribute : Attribute
    {
        /// <summary>The unit named <paramref name="unitName"/>.</summary>
        public UnitAttribute(string unitName)
        {
            UnitName = unitName;
        }

        /// <summary>The unit.</summary>
        public string UnitName { get; set; }
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

    /// <summary>Maps IDL <c>@shared</c> onto generated members
    /// (idl4-cpp / dds-psm-cxx §8.1.5: a pointer / reference-type member).
    /// C# has no <c>shared_ptr</c> equivalent — the member is emitted as a
    /// reference type and carries this marker.</summary>
    [AttributeUsage(
        AttributeTargets.Property | AttributeTargets.Field,
        AllowMultiple = false)]
    public sealed class SharedAttribute : Attribute
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
