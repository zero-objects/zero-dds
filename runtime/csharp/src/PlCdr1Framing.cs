// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
using System;

namespace ZeroDds.Wire
{
    /// <summary>
    /// Where the parameter id being written or read sits.
    /// </summary>
    /// <remarks>
    /// The C# half of <c>zerodds_ast_layout::WirePosition</c>. An argument and
    /// not a field, because one stream carries several positions at once: a
    /// setting made when the writer was built could not answer for all of
    /// them.
    /// </remarks>
    public enum WirePosition
    {
        /// <summary>A member's own parameter id.</summary>
        Member,

        /// <summary>An element of an array or a sequence, which has no parameter id.</summary>
        Element,

        /// <summary>The <c>PID_LIST_END</c> that closes a parameter list.</summary>
        Sentinel,

        /// <summary>The <c>PID_EXTENDED</c> marker that opens an extended header.</summary>
        ExtendedMarker,
    }

    /// <summary>
    /// What flags a PL_CDR1 parameter id carries, asked per position.
    /// </summary>
    /// <remarks>
    /// XTypes 1.3 §7.4.1.2 splits the sixteen-bit field: the low fourteen bits
    /// are the id, bit 14 is <c>PID_MUST_UNDERSTAND</c> and bit 15 is
    /// <c>PID_IMPL_EXTENSION</c>. The clause says nothing about what flags
    /// <c>PID_LIST_END</c> carries, and the four measured implementations do
    /// not agree — <c>struct-mutable</c> and
    /// <c>evolution-mutable-mustunderstand</c> under XCDR1:
    /// <code>
    ///     Cyclone   member as declared    sentinel 7f02
    ///     RTI       member as declared    sentinel 7f02
    ///     Fast DDS  member never          sentinel 3f02
    ///     OpenDDS   member c0xx, always   sentinel 3f02
    /// </code>
    /// The sentinel's answer cannot be read off the member's, which is what
    /// makes <see cref="WirePosition.Sentinel"/> a position on this axis
    /// rather than a property of the list.
    /// </remarks>
    public enum PidFlagPolicy
    {
        /// <summary>
        /// A bare member id, <c>PID_MUST_UNDERSTAND</c> on the sentinel and on
        /// the extended marker. Cyclone DDS and RTI Connext 7.7. The default.
        /// </summary>
        SentinelMustUnderstand,

        /// <summary>Bare everywhere. Fast DDS.</summary>
        Bare,

        /// <summary>
        /// Both flags on every member id whatever the member declared, a bare
        /// sentinel, and both flags on the extended marker. OpenDDS.
        /// </summary>
        ImplExtensionOnMembers,
    }

    /// <summary>What a <see cref="PidFlagPolicy"/> answers.</summary>
    public static class PidFlagPolicies
    {
        /// <summary>The name <c>PidFlagPolicy::as_str</c> gives.</summary>
        public static string AsString(PidFlagPolicy policy)
        {
            switch (policy)
            {
                case PidFlagPolicy.SentinelMustUnderstand: return "sentinel-must-understand";
                case PidFlagPolicy.Bare: return "bare";
                default: return "impl-extension-members";
            }
        }

        /// <summary>
        /// The flag bits a parameter id carries at <paramref name="position"/>.
        /// </summary>
        /// <remarks>
        /// Only the member, the sentinel and the extended marker have a
        /// parameter id at all; <see cref="WirePosition.Element"/> answers
        /// zero.
        /// </remarks>
        public static int FlagsAt(PidFlagPolicy policy, WirePosition position)
        {
            switch (policy)
            {
                case PidFlagPolicy.SentinelMustUnderstand:
                    if (position == WirePosition.Sentinel
                        || position == WirePosition.ExtendedMarker)
                    {
                        return Writer.PidMustUnderstand;
                    }

                    return 0;
                case PidFlagPolicy.Bare:
                    return 0;
                default:
                    if (position == WirePosition.Member
                        || position == WirePosition.ExtendedMarker)
                    {
                        return Writer.PidImplExtension | Writer.PidMustUnderstand;
                    }

                    return 0;
            }
        }

        /// <summary>The named policy, or <c>null</c> for anything else.</summary>
        public static PidFlagPolicy? Parse(string text)
        {
            switch (text)
            {
                case "sentinel-must-understand": return PidFlagPolicy.SentinelMustUnderstand;
                case "bare": return PidFlagPolicy.Bare;
                case "impl-extension-members": return PidFlagPolicy.ImplExtensionOnMembers;
                default: return null;
            }
        }
    }

    /// <summary>
    /// How a PL_CDR1 parameter list is framed.
    /// </summary>
    /// <remarks>
    /// The three ways the measured implementations differ inside an XCDR1
    /// sample. Which header <i>form</i> each member takes is the fourth, and
    /// that one is in the layout plan because it changes the member's extent.
    ///
    /// Immutable. The <c>With…</c> methods return a new framing rather than
    /// changing one a writer already holds: a wire-affecting setting that
    /// could be changed halfway through a sample would produce bytes no reader
    /// can frame.
    /// </remarks>
    public sealed class PlCdr1Framing : IEquatable<PlCdr1Framing>
    {
        /// <summary>
        /// A framing with every axis named.
        /// </summary>
        /// <param name="pidFlags">
        /// What flags a parameter id carries, asked per position — a member's
        /// id, <c>PID_LIST_END</c> and the <c>PID_EXTENDED</c> marker. One
        /// setting for all three, because no position's answer can be read off
        /// another's.
        /// </param>
        /// <param name="highestShortId">
        /// The highest member id a short parameter header carries. §7.4.1.2.2
        /// stops at <c>0x3F00</c> inclusive and reserves the rest of the low
        /// fourteen bits; one implementation carries all fourteen, and a
        /// reader holding the clause's threshold as a constant reports
        /// <c>@id(16383)</c> as reserved and drops the member. Read-side only.
        /// </param>
        /// <param name="padDeclaredLength">
        /// Whether a <c>@mutable</c> member's declared length covers its
        /// trailing padding.
        /// </param>
        /// <param name="padDeclaredOptionalLength">
        /// The same question for an <c>@optional</c> member of a type that is
        /// not <c>@mutable</c>. A separate axis because Cyclone answers the
        /// two differently.
        /// </param>
        /// <param name="memberMustUnderstand">
        /// Whether a member's own must-understand bit goes out. §7.4.1.2.2
        /// makes it unconditional and two of the three write it.
        /// </param>
        /// <param name="padAfterValue">
        /// Whether the parameter that just ended writes its own trailing
        /// padding to the next four-byte boundary. Every header writer aligns
        /// itself, so the two readings only part at the end of the stream:
        /// RTI pads after a final <c>@optional</c> parameter, Fast DDS does
        /// not.
        /// </param>
        public PlCdr1Framing(
            PidFlagPolicy pidFlags,
            int highestShortId,
            bool padDeclaredLength,
            bool padDeclaredOptionalLength,
            bool memberMustUnderstand,
            bool padAfterValue)
        {
            if (highestShortId < 0 || highestShortId > 0x3FFF)
            {
                throw new ArgumentOutOfRangeException(
                    nameof(highestShortId),
                    "a short parameter id inside the low fourteen bits");
            }

            PidFlags = pidFlags;
            HighestShortId = highestShortId;
            PadDeclaredLength = padDeclaredLength;
            PadDeclaredOptionalLength = padDeclaredOptionalLength;
            MemberMustUnderstand = memberMustUnderstand;
            PadAfterValue = padAfterValue;
        }

        /// <summary>The clause's framing, which is this project's default dialect.</summary>
        public static PlCdr1Framing Defaults()
        {
            return new PlCdr1Framing(
                PidFlagPolicy.SentinelMustUnderstand,
                Writer.PidShortMaxId,
                false,
                false,
                true,
                true);
        }

        /// <summary>What flags a parameter id carries, asked per position.</summary>
        public PidFlagPolicy PidFlags { get; private set; }

        /// <summary>The highest member id a short parameter header carries.</summary>
        public int HighestShortId { get; private set; }

        /// <summary>Whether a <c>@mutable</c> member's declared length covers padding.</summary>
        public bool PadDeclaredLength { get; private set; }

        /// <summary>The same question for a non-mutable type's <c>@optional</c> member.</summary>
        public bool PadDeclaredOptionalLength { get; private set; }

        /// <summary>Whether a member's own must-understand bit goes out.</summary>
        public bool MemberMustUnderstand { get; private set; }

        /// <summary>Whether the ended parameter writes its own trailing padding.</summary>
        public bool PadAfterValue { get; private set; }

        /// <summary>Whether a parameter in this role declares a padded length.</summary>
        public bool PadsDeclaredLength(ParameterRole role)
        {
            return role == ParameterRole.Optional
                ? PadDeclaredOptionalLength
                : PadDeclaredLength;
        }

        /// <summary>The same framing with another flag policy.</summary>
        public PlCdr1Framing WithPidFlags(PidFlagPolicy flags)
        {
            return new PlCdr1Framing(
                flags,
                HighestShortId,
                PadDeclaredLength,
                PadDeclaredOptionalLength,
                MemberMustUnderstand,
                PadAfterValue);
        }

        /// <summary>The same framing with another short-id ceiling.</summary>
        public PlCdr1Framing WithHighestShortId(int id)
        {
            return new PlCdr1Framing(
                PidFlags,
                id,
                PadDeclaredLength,
                PadDeclaredOptionalLength,
                MemberMustUnderstand,
                PadAfterValue);
        }

        /// <summary>The same framing with another answer for a <c>@mutable</c> member.</summary>
        public PlCdr1Framing WithPadDeclaredLength(bool pad)
        {
            return new PlCdr1Framing(
                PidFlags,
                HighestShortId,
                pad,
                PadDeclaredOptionalLength,
                MemberMustUnderstand,
                PadAfterValue);
        }

        /// <summary>The same framing with another answer for an <c>@optional</c> member.</summary>
        public PlCdr1Framing WithPadDeclaredOptionalLength(bool pad)
        {
            return new PlCdr1Framing(
                PidFlags,
                HighestShortId,
                PadDeclaredLength,
                pad,
                MemberMustUnderstand,
                PadAfterValue);
        }

        /// <summary>The same framing writing, or not writing, a member's own flag.</summary>
        public PlCdr1Framing WithMemberMustUnderstand(bool write)
        {
            return new PlCdr1Framing(
                PidFlags,
                HighestShortId,
                PadDeclaredLength,
                PadDeclaredOptionalLength,
                write,
                PadAfterValue);
        }

        /// <inheritdoc/>
        public bool Equals(PlCdr1Framing other)
        {
            return other != null
                && PidFlags == other.PidFlags
                && HighestShortId == other.HighestShortId
                && PadDeclaredLength == other.PadDeclaredLength
                && PadDeclaredOptionalLength == other.PadDeclaredOptionalLength
                && MemberMustUnderstand == other.MemberMustUnderstand
                && PadAfterValue == other.PadAfterValue;
        }

        /// <inheritdoc/>
        public override bool Equals(object other)
        {
            return Equals(other as PlCdr1Framing);
        }

        /// <inheritdoc/>
        public override int GetHashCode()
        {
            int hash = (int)PidFlags;
            hash = (hash * 31) + HighestShortId;
            hash = (hash * 31) + (PadDeclaredLength ? 1 : 0);
            hash = (hash * 31) + (PadDeclaredOptionalLength ? 1 : 0);
            hash = (hash * 31) + (MemberMustUnderstand ? 1 : 0);
            hash = (hash * 31) + (PadAfterValue ? 1 : 0);
            return hash;
        }

        /// <inheritdoc/>
        public override string ToString()
        {
            return "PlCdr1Framing[" + PidFlagPolicies.AsString(PidFlags)
                + ", highestShortId=0x" + HighestShortId.ToString(
                    "x", System.Globalization.CultureInfo.InvariantCulture)
                + ", padDeclaredLength=" + PadDeclaredLength
                + ", padDeclaredOptionalLength=" + PadDeclaredOptionalLength
                + ", memberMustUnderstand=" + MemberMustUnderstand
                + ", padAfterValue=" + PadAfterValue + "]";
        }
    }
}
