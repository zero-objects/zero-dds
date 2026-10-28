// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What can go wrong on the wire.

use alloc::string::String;

/// Why a read failed, or a write refused.
///
/// Every variant names what was expected and where. A reader that
/// returned a zero for a truncated stream would hand the caller a value
/// that was never sent; a writer that narrowed a value it cannot carry
/// would send one nobody wrote. `OutOfRange` is the variant both use —
/// `at` is the offset read from on the way in and the offset the value
/// would have gone to on the way out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// The stream ended before the value did.
    Truncated {
        /// Where the read started.
        at: usize,
        /// How many bytes it needed.
        needed: usize,
        /// How many were left.
        available: usize,
    },
    /// A length field claims more than the stream holds.
    ///
    /// Separate from `Truncated`: a length that cannot be true is a
    /// malformed stream, not a short one, and allocating for it is how a
    /// reader turns a bad message into an out-of-memory.
    ImpossibleLength {
        /// Where the length was read.
        at: usize,
        /// What it claimed.
        claimed: usize,
        /// How many bytes remain.
        available: usize,
    },
    /// A string held bytes that are not UTF-8.
    NotUtf8 {
        /// Where the string started.
        at: usize,
    },
    /// A value outside what its type admits.
    OutOfRange {
        /// Where it was read.
        at: usize,
        /// What was expected.
        expected: String,
    },
    /// A value the receiving type cannot hold, under a position whose
    /// `try_construct` is `DISCARD`.
    ///
    /// DDS-XTypes 1.3 §7.2.2.7 and §7.2.4.1: "the default behavior is to
    /// discard the O2 object to avoid misinterpretation." §7.2.3 says
    /// the sample "can be safely ignored (filtered out) by the
    /// receiver".
    ///
    /// Kept apart from every other variant on purpose, and the
    /// distinction is measured. Cyclone's path is the clearest witness:
    /// `dds_stream_normalize` fails inside `ddsi_serdata_from_ser`,
    /// which returns `NULL`, and delivery ends at
    /// `remote_on_delivery_failure_fastpath` — the sample never becomes
    /// a sample. **A discarded sample is an absence, not an error.** The
    /// stream is well formed and the next sample may well fit; every
    /// other variant here says the *stream* is wrong. The reflective
    /// column spells it `ProbeError::Discarded`, the C column
    /// `ZDW_DISCARDED` and the Python column `Discarded`.
    Discarded {
        /// Where the value that does not fit began.
        at: usize,
        /// What the receiving type can hold.
        expected: String,
        /// What arrived.
        found: String,
    },
    /// The stream is well formed and framed the way another
    /// implementation frames it, not the way this reader is configured.
    ///
    /// Separate from [`Self::OutOfRange`] on purpose. A reader given the
    /// wrong dialect and a reader given a corrupt stream both stop, and
    /// only one of them is fixed by changing a flag. Telling a user
    /// "expected a four-byte parameter length" when the answer is
    /// `--dialect fastdds` costs an afternoon; naming the form the bytes
    /// do fit costs nothing.
    WrongFraming {
        /// Where the mismatch was found.
        at: usize,
        /// What the configured framing calls for.
        expected: String,
        /// What the stream has instead.
        found: String,
        /// The setting whose other value the stream fits, when one does
        /// — `Some("--pl-cdr1-headers short-true-length")`. `None` when
        /// no configuration would have produced these bytes, which
        /// makes it a malformed stream after all.
        fits: Option<&'static str>,
    },
}

impl WireError {
    /// A refusal raised from outside the runtime.
    ///
    /// The runtime owns the wire and the generated code owns the type,
    /// and some refusals are the type's: a member the stream says is
    /// absent where the generated field cannot hold an absence, a value
    /// that names no enumerator. `runtime/c` already has
    /// `zdw_reader_refuse` for exactly this; the Rust column had no way
    /// to say it and read on instead, which puts every following member
    /// at the wrong offset and reports success.
    #[must_use]
    pub fn refused(at: usize, expected: &str) -> Self {
        Self::OutOfRange {
            at,
            expected: String::from(expected),
        }
    }

    /// Whether this is the absence [`WireError::Discarded`] describes
    /// rather than a statement that the stream is malformed.
    ///
    /// The one question a subscriber has to be able to ask: a discarded
    /// sample is dropped and the next one read, a malformed stream is
    /// reported. §7.2.4.1 against §7.2.3.
    #[must_use]
    pub const fn is_discarded(&self) -> bool {
        matches!(self, Self::Discarded { .. })
    }
}

impl core::fmt::Display for WireError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated {
                at,
                needed,
                available,
            } => write!(f, "at {at}: {needed} bytes needed, {available} available"),
            Self::ImpossibleLength {
                at,
                claimed,
                available,
            } => write!(
                f,
                "at {at}: a length of {claimed} with {available} bytes left"
            ),
            Self::NotUtf8 { at } => write!(f, "at {at}: the string is not UTF-8"),
            Self::Discarded {
                at,
                expected,
                found,
            } => write!(
                f,
                "at {at}: the sample is discarded — this type holds {expected} and the stream \
                 carried {found}"
            ),
            Self::OutOfRange { at, expected } => write!(f, "at {at}: expected {expected}"),
            Self::WrongFraming {
                at,
                expected,
                found,
                fits,
            } => match fits {
                Some(setting) => write!(
                    f,
                    "at {at}: this reader is configured for {expected} and the stream has \
                     {found}, which is what {setting} writes"
                ),
                None => write!(
                    f,
                    "at {at}: this reader is configured for {expected} and the stream has \
                     {found}, which no setting writes"
                ),
            },
        }
    }
}

impl core::error::Error for WireError {}
