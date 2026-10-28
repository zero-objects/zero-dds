// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
namespace ZeroDds.Wire
{
    /// <summary>A DHEADER waiting for its length.</summary>
    /// <remarks>
    /// A <c>readonly struct</c> and not a class, unlike the Java column's
    /// nested classes. Two reasons, and both are C#'s: a generated codec
    /// declares one of these per framed value and a struct costs no
    /// allocation, and a local of a struct type is definitely assigned by
    /// <c>= default</c>, which is what the generated <c>EmHeaderRead zd_h =
    /// default;</c> at the top of a <c>@mutable</c> decoder relies on.
    /// </remarks>
    public readonly struct DHeader
    {
        /// <summary>Where the four-byte placeholder sits.</summary>
        public readonly int At;

        /// <summary>Whether there is one at all.</summary>
        public readonly bool Present;

        /// <summary>A DHEADER at <paramref name="at"/>.</summary>
        public DHeader(int at, bool present)
        {
            At = at;
            Present = present;
        }
    }

    /// <summary>An EMHEADER waiting for its NEXTINT, when it has one.</summary>
    public readonly struct EmHeader
    {
        /// <summary>Where the NEXTINT placeholder sits, when there is one.</summary>
        public readonly int NextInt;

        /// <summary>Whether a placeholder was reserved.</summary>
        public readonly bool HasNextInt;

        /// <summary>
        /// Where the header word itself sits, so a short length code can be
        /// patched into it once the value's length is known.
        /// </summary>
        public readonly int Word;

        /// <summary>
        /// The code the header was written with, so that
        /// <c>FinishEmheader</c> can hold the member's value to the length
        /// the code declares. Codes 0 to 3 are lengths; anything else
        /// declares nothing of its own.
        /// </summary>
        public readonly int LengthCode;

        /// <summary>An EMHEADER whose word sits at <paramref name="word"/>.</summary>
        public EmHeader(int nextInt, bool hasNextInt, int word, int lengthCode)
        {
            NextInt = nextInt;
            HasNextInt = hasNextInt;
            Word = word;
            LengthCode = lengthCode;
        }
    }

    /// <summary>A parameter header waiting for its length.</summary>
    public readonly struct Pid
    {
        /// <summary>Where the length field sits.</summary>
        public readonly int At;

        /// <summary>Whether the header was written in the extended form.</summary>
        public readonly bool Extended;

        /// <summary>The alignment origin this parameter replaced.</summary>
        public readonly int SavedOrigin;

        /// <summary>Which of §7.4.1.2's two roles it stands in.</summary>
        public readonly ParameterRole Role;

        /// <summary>
        /// The member id, kept so that a short header can be rewritten as an
        /// extended one when the value turns out not to fit sixteen bits. The
        /// must-understand bit is in a different word in the two forms.
        /// </summary>
        public readonly int Id;

        /// <summary>Whether the member declared must-understand.</summary>
        public readonly bool MustUnderstand;

        /// <summary>A parameter header whose length field sits at <paramref name="at"/>.</summary>
        public Pid(
            int at,
            bool extended,
            int savedOrigin,
            ParameterRole role,
            int id,
            bool mustUnderstand)
        {
            At = at;
            Extended = extended;
            SavedOrigin = savedOrigin;
            Role = role;
            Id = id;
            MustUnderstand = mustUnderstand;
        }
    }

    /// <summary>
    /// Everything a two-pass read has to put back.
    /// </summary>
    /// <remarks>
    /// One caller: a <c>map</c> under <c>rule-15-omits</c>, where a delimited
    /// map states no entry count and the entries are read once to count them
    /// and again into storage sized from that count. .NET allocates its own
    /// values, so there is no heap mark to carry — the cursor, the stream
    /// origin and the status are the whole of it.
    /// </remarks>
    public readonly struct ReaderMark
    {
        /// <summary>Where the cursor was.</summary>
        public readonly int At;

        /// <summary>Where alignment was counted from.</summary>
        public readonly int Origin;

        /// <summary>How the read was going.</summary>
        public readonly Status Status;

        /// <summary>A mark of the three.</summary>
        public ReaderMark(int at, int origin, Status status)
        {
            At = at;
            Origin = origin;
            Status = status;
        }
    }

    /// <summary>An EMHEADER as read.</summary>
    public readonly struct EmHeaderRead
    {
        /// <summary>The 28-bit member id.</summary>
        public readonly uint MemberId;

        /// <summary>The length code, 0 to 7.</summary>
        public readonly int LengthCode;

        /// <summary>Whether the writer set the member's must-understand flag.</summary>
        public readonly bool MustUnderstand;

        /// <summary>The NEXTINT, for the codes that carry one.</summary>
        public readonly uint NextInt;

        /// <summary>Whether there was a NEXTINT at all.</summary>
        public readonly bool HasNextInt;

        /// <summary>An EMHEADER taken apart.</summary>
        public EmHeaderRead(
            uint memberId,
            int lengthCode,
            bool mustUnderstand,
            uint nextInt,
            bool hasNextInt)
        {
            MemberId = memberId;
            LengthCode = lengthCode;
            MustUnderstand = mustUnderstand;
            NextInt = nextInt;
            HasNextInt = hasNextInt;
        }

        /// <summary>
        /// How many bytes the member occupies after the header word, NEXTINT
        /// included where there is one.
        /// </summary>
        /// <remarks>
        /// What a reader skipping an unknown member advances by, and what a
        /// reader of a known one checks itself against. XTypes 1.3 §7.4.3.4.2:
        /// codes 0 to 3 state a size of one, two, four or eight bytes outright;
        /// code 4 is followed by a NEXTINT holding the member's length; and
        /// codes 5, 6 and 7 have a NEXTINT that is <i>also</i> the member's own
        /// leading word, so the member spans that word plus the length it
        /// states, times one, four or eight.
        /// </remarks>
        public long Extent
        {
            get
            {
                if (LengthCode <= 3)
                {
                    return 1L << LengthCode;
                }

                if (LengthCode == 4)
                {
                    return NextInt;
                }

                long scale = LengthCode == 5 ? 1L : LengthCode == 6 ? 4L : 8L;
                return 4L + ((long)NextInt * scale);
            }
        }
    }

    /// <summary>A parameter header as read.</summary>
    public readonly struct PidRead
    {
        /// <summary>The member id, with the flag bits taken off.</summary>
        public readonly uint Pid;

        /// <summary>The declared length.</summary>
        public readonly long Length;

        /// <summary>Whether this is the <c>PID_LIST_END</c> sentinel.</summary>
        public readonly bool End;

        /// <summary>Whether the writer set the member's must-understand flag.</summary>
        public readonly bool MustUnderstand;

        /// <summary>
        /// Whether the writer set <c>FLAG_IMPL_EXTENSION</c>.
        /// </summary>
        /// <remarks>
        /// §7.4.1.2.2: the parameter is the writing implementation's own, and a
        /// user-defined member "will never set the FLAG_IMPL_EXTENSION bit".
        /// Masking it off with the rest made a vendor-private parameter
        /// indistinguishable from the member with the same low fourteen bits.
        /// </remarks>
        public readonly bool ImplExtension;

        /// <summary>
        /// Whether the short-form id is inside §7.4.1.2.2's reserved range
        /// <c>0x3F01</c>-<c>0x3FFF</c>, which no member id occupies.
        /// </summary>
        public readonly bool Reserved;

        /// <summary>
        /// <c>PID_IGNORE</c>, which §7.4.1.2.2 says every consumer shall
        /// ignore — with no exception for the must-understand bit.
        /// </summary>
        public readonly bool Ignore;

        /// <summary>Which of §7.4.1.2's two roles the caller read it in.</summary>
        public readonly ParameterRole Role;

        /// <summary>The alignment origin this parameter replaced.</summary>
        public readonly int SavedOrigin;

        /// <summary>
        /// Where the parameter's value starts — the cursor right after the
        /// header. The length check in <c>FinishPidRead</c> measures the
        /// consumed value against this; it used to reconstruct the header
        /// position from <c>SavedOrigin</c> plus a *guessed* header size,
        /// which broke for every extended header with a small id — the
        /// AlwaysExtended dialect writes twelve octets where the guess said
        /// four, and an <c>@optional</c> member of a non-mutable type starts
        /// nowhere near the enclosing origin at all.
        /// </summary>
        public readonly int ValueStart;

        /// <summary>A parameter header taken apart.</summary>
        public PidRead(
            uint pid,
            long length,
            bool end,
            bool mustUnderstand,
            bool implExtension,
            bool reserved,
            bool ignore,
            ParameterRole role,
            int savedOrigin,
            int valueStart)
        {
            Pid = pid;
            Length = length;
            End = end;
            MustUnderstand = mustUnderstand;
            ImplExtension = implExtension;
            Reserved = reserved;
            Ignore = ignore;
            Role = role;
            SavedOrigin = savedOrigin;
            ValueStart = valueStart;
        }

        /// <summary>
        /// Whether this parameter can be a member of the reading type.
        /// </summary>
        /// <remarks>
        /// False for a vendor-private parameter and for a reserved id. Both are
        /// legal for a peer to send and neither is a member, so both take the
        /// unknown-parameter path — skipped, or the sample discarded when the
        /// writer set must-understand. §7.4.1.2.2, §7.2.2.4.4.4.6.
        /// </remarks>
        public bool IsUserMember
        {
            get { return !ImplExtension && !Reserved && !Ignore; }
        }
    }

    /// <summary>
    /// How the read is going.
    /// </summary>
    /// <remarks>
    /// The counterpart of <c>zdw_status</c> in <c>runtime/c</c>, and it exists
    /// for the same reason: the runtime owns the wire and the generated decoder
    /// owns the type, and some refusals are the type's — an integer that names
    /// no enumerator, a bitmask bit the declaration does not have, a sequence
    /// longer than its bound, a member that did not arrive. The reader cannot
    /// see any of them.
    ///
    /// A malformed <i>stream</i> still raises a <see cref="WireException"/>: a
    /// length that runs past the end is not something a caller can carry on
    /// from, and an exception names where it happened without every call site
    /// having to ask.
    /// </remarks>
    public enum Status
    {
        /// <summary>Nothing has been refused.</summary>
        Ok,

        /// <summary>
        /// §7.2.2.7: the sample is dropped and the receiver carries on. Not a
        /// failure of the stream — the writer's type and the reader's differ,
        /// and a caller that cannot tell that from a broken peer cannot tell a
        /// narrowed type from a corrupt one.
        /// </summary>
        Discarded,

        /// <summary>A length no stream that long can carry.</summary>
        ImpossibleLength,

        /// <summary>A value outside what the type admits.</summary>
        OutOfRange,

        /// <summary>The stream ended where a value was expected.</summary>
        Truncated,
    }
}
