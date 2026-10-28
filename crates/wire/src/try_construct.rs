// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a receiving type does with a value it cannot hold.
//!
//! DDS-XTypes 1.3 §7.2.2.7 and §7.3.1.2.1.12. The behaviour belongs to
//! the *position* — a member, a collection element, a map key, a union
//! case, the discriminator — and not to the type, because one
//! `typedef string<5>` may be used at two positions that want different
//! answers. So it arrives here as an argument, exactly as it arrives at
//! the C runtime as `zdw_try_construct`.
//!
//! The bound is not on the wire. Both sides serialise and deserialise
//! the same bytes and the whole difference is what the receiving side
//! decides, which is why
//! `crates/ast-layout/tests/fixtures/try_construct_behaviour.md` records
//! behaviours rather than byte strings. **OpenDDS is the only one of the
//! five implementations that implements USE_DEFAULT and TRIM**; Cyclone,
//! RTI 7.7 and omniORB discard whatever the annotation says, and Fast
//! DDS cannot generate code for the annotation in any position.
//!
//! Every caller reads the whole value off the wire *before* it gets
//! here. A reader that stopped at its own bound would leave the rest in
//! the stream and put every following member at the wrong offset —
//! OpenDDS consumes the whole twelve-character string before it defaults
//! or truncates, and the members after it come back correct, which is
//! the property that matters.

use crate::WireError;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The three behaviours §7.3.1.2.1.12 names.
///
/// The annotation's own parameter defaults to `USE_DEFAULT`, so a bare
/// `@try_construct` is not an absent one: an *absent* annotation is
/// DISCARD (§7.2.2.7) and a *valueless* one is USE_DEFAULT
/// (§7.3.1.2.1.12). Which of the two a declaration wrote is the front
/// end's question; by the time it reaches here it is one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TryConstruct {
    /// The whole sample is discarded. The default when no annotation is
    /// written.
    Discard,
    /// The member takes its default and the sample survives.
    UseDefault,
    /// An over-long string, wstring, sequence or map is truncated to
    /// what the receiver's type holds.
    Trim,
}

/// What a narrowing decided.
///
/// `Default` and not the default value itself: Table 9's default for a
/// type is a *language* value the runtime cannot spell — an empty
/// `Vec`, a struct literal with a value per member, an enumeration's
/// first literal — and the generated code has it already, from the same
/// table the other three columns derive theirs from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Narrowed<T> {
    /// Keep this value. It either fitted or was trimmed to fit.
    Value(T),
    /// Substitute the receiving member's default value.
    Default,
}

/// A string that has been read whole, against the receiver's bound.
///
/// The bound counts **octets**, which is what `string<N>` means to every
/// peer that has to hold the value: `char[N + 1]` in the C mapping of
/// all four DDS implementations, `fixed_string<N>` in Fast DDS's.
/// Counting Unicode scalar values instead would accept a five-character
/// value of ten octets that no C peer can hold.
///
/// A `wstring<N>` bound counts UTF-16 code units instead and goes to
/// [`fit_wstring`], which is a second function and not an argument: the
/// two bounds count different things, and one function taking "octets
/// or units" would be a call site deciding which by getting an
/// argument right.
///
/// # Errors
/// [`WireError::Discarded`] under [`TryConstruct::Discard`], and
/// [`WireError::OutOfRange`] for a [`TryConstruct::Trim`] the front end
/// should have refused — Table 11 makes TRIM on a type that cannot be
/// trimmed "unspecified and may be treated as an error", Cyclone refuses
/// such a declaration by name, and reaching here means a model that did
/// not come through the front end.
pub fn fit_string(
    at: usize,
    value: String,
    bound: usize,
    behaviour: TryConstruct,
) -> Result<Narrowed<String>, WireError> {
    if value.len() <= bound {
        return Ok(Narrowed::Value(value));
    }
    match behaviour {
        TryConstruct::Discard => Err(WireError::Discarded {
            at,
            expected: format!("at most {bound} octets"),
            found: format!("{} of them", value.len()),
        }),
        TryConstruct::UseDefault => Ok(Narrowed::Default),
        // §7.3.1.2.1.12.2 keeps the order and takes a prefix: "the
        // string 'Hello'" out of "Hello World". Never inside a
        // character — half a UTF-8 sequence is not text, and a C peer
        // holding `char[bound + 1]` cannot store half of one either.
        TryConstruct::Trim => {
            let mut end = bound;
            while end > 0 && !value.is_char_boundary(end) {
                end -= 1;
            }
            let mut trimmed = value;
            trimmed.truncate(end);
            Ok(Narrowed::Value(trimmed))
        }
    }
}

/// A sequence that has been read whole, against the receiver's bound.
///
/// Table 17, the `SEQUENCE_TYPE` row: "An object O2 of type T2 can
/// construct T1 if and only if O2.length <= T1.length."
/// §7.3.1.2.1.12.3 is the TRIM case and it takes the **first** elements
/// — "a sequence of 4 longs containing the first four elements of
/// O2.member".
///
/// # Errors
/// As [`fit_string`].
pub fn fit_sequence<T>(
    at: usize,
    value: Vec<T>,
    bound: usize,
    behaviour: TryConstruct,
) -> Result<Narrowed<Vec<T>>, WireError> {
    if value.len() <= bound {
        return Ok(Narrowed::Value(value));
    }
    match behaviour {
        TryConstruct::Discard => Err(WireError::Discarded {
            at,
            expected: format!("at most {bound} elements"),
            found: format!("{}", value.len()),
        }),
        TryConstruct::UseDefault => Ok(Narrowed::Default),
        TryConstruct::Trim => {
            let mut trimmed = value;
            trimmed.truncate(bound);
            Ok(Narrowed::Value(trimmed))
        }
    }
}

/// A map that has been read whole, against the receiver's bound.
///
/// Table 11 names `map` among the forms a TRIM may be written on —
/// "string, wide string, sequence, or map" — and Table 17's
/// `MAP_TYPE` row carries the same `O2.length <= T1.length` the
/// `SEQUENCE_TYPE` row does, counted in **entries**.
///
/// §7.3.1.2.1.12.3 takes the *first* elements. A map has no declaration
/// order once it is in an ordered container, so the first `bound`
/// entries are the `bound` smallest keys — which is the same rule
/// [`crate::Writer::write_map`] writes under and not a second one.
///
/// # Errors
/// As [`fit_string`].
pub fn fit_map<K: Ord, V>(
    at: usize,
    value: BTreeMap<K, V>,
    bound: usize,
    behaviour: TryConstruct,
) -> Result<Narrowed<BTreeMap<K, V>>, WireError> {
    if value.len() <= bound {
        return Ok(Narrowed::Value(value));
    }
    match behaviour {
        TryConstruct::Discard => Err(WireError::Discarded {
            at,
            expected: format!("at most {bound} entries"),
            found: format!("{}", value.len()),
        }),
        TryConstruct::UseDefault => Ok(Narrowed::Default),
        TryConstruct::Trim => Ok(Narrowed::Value(
            value.into_iter().take(bound).collect::<BTreeMap<K, V>>(),
        )),
    }
}

/// A map held as its entries (XTypes 1.3 §7.5.1.3.6) that has been read whole, against the
/// receiver's bound, which counts entries.
///
/// A trim keeps the first `bound` entries, in the order the stream carried them.
///
/// # Errors
/// As [`fit_string`].
pub fn fit_map_entries<K, V>(
    at: usize,
    mut value: alloc::vec::Vec<crate::MapEntry<K, V>>,
    bound: usize,
    behaviour: TryConstruct,
) -> Result<Narrowed<alloc::vec::Vec<crate::MapEntry<K, V>>>, WireError> {
    if value.len() <= bound {
        return Ok(Narrowed::Value(value));
    }
    match behaviour {
        TryConstruct::Discard => Err(WireError::Discarded {
            at,
            expected: format!("at most {bound} entries"),
            found: format!("{}", value.len()),
        }),
        TryConstruct::UseDefault => Ok(Narrowed::Default),
        TryConstruct::Trim => {
            value.truncate(bound);
            Ok(Narrowed::Value(value))
        }
    }
}

/// A `wstring` that has been read whole, against the receiver's bound.
///
/// The bound counts **UTF-16 code units**, which is what `wstring<N>`
/// means to every peer that has to hold the value — an `N`-unit buffer
/// in all four DDS implementations' mappings, and what XTypes 1.3
/// §7.4.1.1.2's own length is in. Counting Unicode scalar values
/// instead would accept a five-character value of six code units that
/// no peer can hold; counting octets would make one declaration mean
/// two different capacities under the two widths
/// `zerodds_wire::WstringUnitWidth` carries.
///
/// The value arrives as a `String` because that is what
/// [`crate::Reader::read_wstring`] answers and what the generated field
/// holds; the count is over its UTF-16 encoding and not over its
/// octets.
///
/// # Errors
/// As [`fit_string`].
pub fn fit_wstring(
    at: usize,
    value: String,
    bound: usize,
    behaviour: TryConstruct,
) -> Result<Narrowed<String>, WireError> {
    let units = value.encode_utf16().count();
    if units <= bound {
        return Ok(Narrowed::Value(value));
    }
    match behaviour {
        TryConstruct::Discard => Err(WireError::Discarded {
            at,
            expected: format!("at most {bound} UTF-16 code units"),
            found: format!("{units} of them"),
        }),
        TryConstruct::UseDefault => Ok(Narrowed::Default),
        // §7.3.1.2.1.12.2 keeps the order and takes a prefix. Never
        // inside a character: a lone surrogate is not text, and a peer
        // holding an `N`-unit buffer cannot store half of a pair
        // either. So a bound that falls between the two halves of a
        // surrogate pair takes `bound - 1` units — one short of the
        // bound, and a well-formed string, which is the pair of
        // properties a peer can act on.
        TryConstruct::Trim => {
            let mut kept: Vec<u16> = value.encode_utf16().take(bound).collect();
            if kept
                .last()
                .is_some_and(|unit| (0xD800..0xDC00).contains(unit))
            {
                kept.pop();
            }
            // Every unit came out of a `String` and no surrogate pair is
            // left half-taken, so the re-decode cannot fail; the error
            // arm is the one the type demands rather than a case that
            // can be reached.
            let trimmed = String::from_utf16(&kept).map_err(|_| WireError::OutOfRange {
                at,
                expected: String::from("well-formed UTF-16 after a trim to the receiver's bound"),
            })?;
            Ok(Narrowed::Value(trimmed))
        }
    }
}
