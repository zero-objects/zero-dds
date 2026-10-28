// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Why a reflective operation could not be carried out.
//!
//! One error type for the whole module, because the three things it does
//! — look a type up in the model, build a model out of a `TypeObject`,
//! read a value out of a `DynamicData` — are asked in one sequence by
//! every caller that reads a foreign sample, and three error types would
//! be three conversions at every call site.
//!
//! Nothing here is a fallback. Every variant names what was asked for and
//! what was there instead; a reflective API that answered a question it
//! could not answer would put a value on the wire nobody wrote.

use alloc::string::String;

use super::descriptor::TypeKind;

/// Why a reflective operation could not be carried out.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DynamicError {
    /// No type of that name is in the model.
    NoSuchType {
        /// The scoped name asked for.
        name: String,
    },
    /// A handle the model does not answer for — a reference the resolver
    /// left `Pending` or `Broken`, or a stale id.
    Unresolved {
        /// Where the reference sat.
        at: String,
    },
    /// The type has no members, and members were asked for.
    NotAnAggregate {
        /// The type asked about.
        type_name: String,
        /// What it is instead.
        kind: TypeKind,
    },
    /// The type declares no member of that name or id.
    NoSuchMember {
        /// The type asked about.
        type_name: String,
        /// The member asked for, by name or as a decimal id.
        member: String,
    },
    /// The type, or a descriptor for one, contradicts itself.
    ///
    /// The consistency rules of §7.5.2.4.10 and §7.5.2.7.7: a union with no discriminator, a
    /// sequence with no element type, an array with no dimensions.
    Inconsistent {
        /// What is wrong.
        detail: String,
    },
    /// Building the model rejected the edit.
    ///
    /// Carries [`zerodds_ast::AstError`]'s rendering rather than the
    /// error: the model's error type is not `PartialEq`, and an error a
    /// test cannot compare is an error a test cannot assert.
    Model {
        /// What the model said.
        detail: String,
    },
    /// A `TypeIdentifier` names a `TypeObject` the registry does not
    /// hold.
    ///
    /// Loud, and never an opaque placeholder type: a member whose type is
    /// unknown makes every member after it land at the wrong offset, and
    /// a placeholder would put that failure at the last member instead of
    /// at the one that caused it.
    UnknownTypeIdentifier {
        /// Where the identifier sat.
        at: String,
    },
    /// A `MinimalTypeObject` was offered where a name is needed.
    ///
    /// §7.3.4.1: the minimal form carries a four-byte hash of each name
    /// where the complete form carries the name. A hash is not
    /// invertible, so a `DynamicType` built from a minimal object would
    /// have to invent every member name it reports — and a member name a
    /// peer never wrote is exactly what a reflective reader must not
    /// produce. §7.6.3.2.2 says as much: the complete form is what
    /// "provides all the information needed to … construct a
    /// `DynamicType`".
    MinimalHasNoNames {
        /// The type asked about, as far as it is known.
        at: String,
    },
    /// The construct has no counterpart in the other representation.
    Unrepresentable {
        /// What was being converted.
        what: String,
        /// Which construct stopped it.
        kind: &'static str,
    },
    /// `RETCODE_BAD_PARAMETER`: an argument the operation cannot take — a
    /// nil argument, an index past the end, a member id the type does not
    /// have (§7.5.2).
    BadParameter {
        /// What was wrong with the argument.
        detail: String,
    },
    /// `RETCODE_NO_DATA`: a qualified lookup that finds no value for the
    /// key (§7.5.2.1: "The 'get' operation shall return with
    /// RETCODE_NO_DATA if no value exists for the given key").
    NoData {
        /// What was looked up.
        detail: String,
    },
    /// `RETCODE_PRECONDITION_NOT_MET`: the object is not in a state the
    /// operation needs — adding a member to an alias whose base type takes
    /// none (Table 55), a second open loan (§7.5.2.11.7).
    PreconditionNotMet {
        /// What was not met.
        detail: String,
    },
    /// `RETCODE_ALREADY_DELETED`: the object was deleted before
    /// (§7.5.2.2.10).
    AlreadyDeleted {
        /// What was deleted.
        what: String,
    },
    /// The type refers to itself, directly or through other types.
    ///
    /// §7.3.4.9 answers a recursive type with a strongly connected
    /// component identifier over the whole cycle, which is a different
    /// computation from a single type's hash and is not implemented —
    /// see [`crate::from_ast`], which refuses the same thing on the way
    /// out for the same reason.
    Recursive {
        /// The declaration the cycle was entered at.
        what: String,
    },
}

impl core::fmt::Display for DynamicError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoSuchType { name } => write!(f, "`{name}` is not in the model"),
            Self::Unresolved { at } => write!(f, "{at}: the reference is not resolved"),
            Self::NotAnAggregate { type_name, kind } => {
                write!(
                    f,
                    "`{type_name}` is {}, which has no members",
                    kind.as_str()
                )
            }
            Self::NoSuchMember { type_name, member } => {
                write!(f, "`{type_name}` declares no member `{member}`")
            }
            Self::Inconsistent { detail } => write!(f, "the type is inconsistent: {detail}"),
            Self::Model { detail } => write!(f, "the model rejected the edit: {detail}"),
            Self::UnknownTypeIdentifier { at } => write!(
                f,
                "{at}: the TypeIdentifier names a TypeObject the registry does not hold"
            ),
            Self::MinimalHasNoNames { at } => write!(
                f,
                "{at}: a MinimalTypeObject carries name hashes and not names, so no \
                 DynamicType can be built from one — §7.6.3.2.2 asks for the complete form"
            ),
            Self::Unrepresentable { what, kind } => {
                write!(f, "{what}: {kind} has no counterpart here")
            }
            Self::BadParameter { detail } => write!(f, "RETCODE_BAD_PARAMETER: {detail}"),
            Self::NoData { detail } => write!(f, "RETCODE_NO_DATA: {detail}"),
            Self::PreconditionNotMet { detail } => {
                write!(f, "RETCODE_PRECONDITION_NOT_MET: {detail}")
            }
            Self::AlreadyDeleted { what } => write!(f, "RETCODE_ALREADY_DELETED: {what}"),
            Self::Recursive { what } => write!(
                f,
                "{what}: recursive, and §7.3.4.9's strongly connected component identifiers \
                 are not implemented"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DynamicError {}

impl DynamicError {
    /// An [`zerodds_ast::AstError`] as a [`Self::Model`].
    pub(super) fn model(error: &zerodds_ast::AstError) -> Self {
        use alloc::string::ToString as _;
        Self::Model {
            detail: error.to_string(),
        }
    }

    /// A [`Self::BadParameter`].
    pub(super) fn bad_parameter(detail: impl Into<String>) -> Self {
        Self::BadParameter {
            detail: detail.into(),
        }
    }

    /// A [`Self::NoData`].
    pub(super) fn no_data(detail: impl Into<String>) -> Self {
        Self::NoData {
            detail: detail.into(),
        }
    }

    /// A [`Self::PreconditionNotMet`].
    pub(super) fn precondition_not_met(detail: impl Into<String>) -> Self {
        Self::PreconditionNotMet {
            detail: detail.into(),
        }
    }

    /// The DDS return code this error stands for, by its name.
    ///
    /// The Dynamic Language Binding states its failures as DDS return
    /// codes; an error without a code of its own is `RETCODE_ERROR`
    /// ("an implementation-specific error"), and a handle the model does
    /// not answer for is `RETCODE_BAD_PARAMETER`.
    #[must_use]
    pub const fn return_code(&self) -> &'static str {
        match self {
            Self::BadParameter { .. }
            | Self::NoSuchType { .. }
            | Self::NoSuchMember { .. }
            | Self::Unresolved { .. }
            | Self::Inconsistent { .. } => "RETCODE_BAD_PARAMETER",
            Self::NoData { .. } => "RETCODE_NO_DATA",
            Self::PreconditionNotMet { .. } | Self::NotAnAggregate { .. } => {
                "RETCODE_PRECONDITION_NOT_MET"
            }
            Self::AlreadyDeleted { .. } => "RETCODE_ALREADY_DELETED",
            _ => "RETCODE_ERROR",
        }
    }

    /// An inconsistency, with its detail.
    pub(super) fn inconsistent(detail: impl Into<String>) -> Self {
        Self::Inconsistent {
            detail: detail.into(),
        }
    }
}
