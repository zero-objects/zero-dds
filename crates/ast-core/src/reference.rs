// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Three-valued references. Spec §2.

use crate::arena::Id;
use crate::id::NodeId;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// A reference from one node to another.
///
/// Deliberately not `Option<Id<T>>`: type discovery delivers types whose
/// targets have not arrived yet, and "absent" and "not resolved yet" are
/// different facts that both need representing. Spec §2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ref<T> {
    /// Points at a live node.
    Resolved(Id<T>),
    /// Names a target that has not been resolved yet.
    Pending(PendingRef),
    /// Resolution was attempted and failed.
    Broken(BreakCause),
}

impl<T> Ref<T> {
    /// The handle, if resolved.
    #[must_use]
    pub const fn resolved(&self) -> Option<Id<T>> {
        match self {
            Self::Resolved(id) => Some(*id),
            Self::Pending(_) | Self::Broken(_) => None,
        }
    }

    /// Whether this reference points at a live node.
    #[must_use]
    pub const fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved(_))
    }

    /// Whether resolution is still outstanding.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(_))
    }

    /// Whether resolution failed.
    #[must_use]
    pub const fn is_broken(&self) -> bool {
        matches!(self, Self::Broken(_))
    }

    /// The lookup key, for the pending-reference index.
    ///
    /// `None` once the reference is resolved or broken. A broken
    /// reference is not waiting on anything and must not be retried
    /// implicitly — a diagnostic already reported must not silently
    /// vanish. Spec §2.
    #[must_use]
    pub const fn pending_key(&self) -> Option<&PendingRef> {
        match self {
            Self::Pending(key) => Some(key),
            Self::Resolved(_) | Self::Broken(_) => None,
        }
    }

    /// The failure cause, if broken.
    #[must_use]
    pub const fn break_cause(&self) -> Option<&BreakCause> {
        match self {
            Self::Broken(cause) => Some(cause),
            Self::Resolved(_) | Self::Pending(_) => None,
        }
    }
}

/// What an unresolved reference names, in whichever vocabulary its
/// frontend uses. Spec §2.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PendingRef {
    /// IDL scoped name, including forward declarations.
    ScopedName(ScopedName),
    /// XTypes type identifier, from discovery over the wire.
    TypeIdentifier(TypeIdentifierKey),
    /// ROS 2 interface name.
    RosTypeName(RosTypeName),
    /// CORBA repository identifier.
    RepositoryId(RepositoryId),
    /// DDS-XML `idref` target.
    XmlRef(XmlRefKey),
}

/// IDL scoped name.
///
/// Ordering is segment-wise so that indices built on it iterate
/// deterministically.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScopedName {
    absolute: bool,
    segments: Vec<String>,
}

impl ScopedName {
    /// Name rooted at the global scope (`::A::B`).
    #[must_use]
    pub const fn absolute(segments: Vec<String>) -> Self {
        Self {
            absolute: true,
            segments,
        }
    }

    /// Name resolved relative to the current scope (`A::B`).
    #[must_use]
    pub const fn relative(segments: Vec<String>) -> Self {
        Self {
            absolute: false,
            segments,
        }
    }

    /// Whether the name is rooted at the global scope.
    #[must_use]
    pub const fn is_absolute(&self) -> bool {
        self.absolute
    }

    /// The name's segments, outermost first.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// The innermost segment, if any.
    #[must_use]
    pub fn last(&self) -> Option<&String> {
        self.segments.last()
    }
}

impl fmt::Display for ScopedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.absolute {
            write!(f, "::")?;
        }
        for (index, segment) in self.segments.iter().enumerate() {
            if index > 0 {
                write!(f, "::")?;
            }
            write!(f, "{segment}")?;
        }
        Ok(())
    }
}

/// Which ROS 2 interface file a name came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RosInterfaceKind {
    /// `.msg`
    Msg,
    /// `.srv`
    Srv,
    /// `.action`
    Action,
}

/// ROS 2 interface name: `package/kind/Name`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RosTypeName {
    /// ROS package.
    pub package: String,
    /// Interface file kind.
    pub kind: RosInterfaceKind,
    /// Interface name.
    pub name: String,
}

/// CORBA repository identifier, for example `IDL:Robot/Pose:1.0`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RepositoryId(pub String);

/// XTypes type identifier in its opaque 16-byte form.
///
/// The model treats it as a lookup key; computing it is the runtime's
/// concern, not this crate's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeIdentifierKey(pub [u8; 16]);

/// DDS-XML cross-reference target.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct XmlRefKey(pub String);

/// Why resolution failed.
///
/// A reference must never sit in `Pending` after a failed attempt — the
/// failure is recorded so it can be reported. Spec §2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreakCause {
    /// Nothing in the model carries that name.
    NotFound {
        /// The name that was looked up.
        what: PendingRef,
    },
    /// More than one node carries that name.
    Ambiguous {
        /// The name that was looked up.
        what: PendingRef,
        /// Every node that matched.
        candidates: Vec<NodeId>,
    },
    /// The target exists but is the wrong kind of node.
    KindMismatch {
        /// Kind the reference required.
        expected: &'static str,
        /// Kind actually found.
        found: &'static str,
    },
    /// Resolving would close a cycle the specification forbids, for
    /// example a struct inheriting from itself.
    IllegalCycle {
        /// Node the cycle passes through.
        through: NodeId,
    },
}

#[cfg(test)]
mod tests {
    // Tests assert on Results directly; a failed expect IS the test failure.
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::arena::Arena;
    use alloc::string::ToString;
    use alloc::vec;

    #[test]
    fn resolved_yields_its_handle_and_the_others_do_not() {
        let mut arena: Arena<u32> = Arena::new();
        let id = arena.insert(1).expect("insert");

        let resolved: Ref<u32> = Ref::Resolved(id);
        let pending: Ref<u32> = Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(vec![
            "Robot".to_string(),
            "Pose".to_string(),
        ])));
        let broken: Ref<u32> = Ref::Broken(BreakCause::NotFound {
            what: PendingRef::RepositoryId(RepositoryId("IDL:Robot/Pose:1.0".to_string())),
        });

        assert_eq!(resolved.resolved(), Some(id));
        assert_eq!(pending.resolved(), None);
        assert_eq!(broken.resolved(), None);
        assert!(resolved.is_resolved());
        assert!(pending.is_pending());
        assert!(broken.is_broken());
    }

    #[test]
    fn pending_key_is_available_for_indexing_but_not_after_breaking() {
        let key = PendingRef::RosTypeName(RosTypeName {
            package: "geometry_msgs".to_string(),
            kind: RosInterfaceKind::Msg,
            name: "Pose".to_string(),
        });
        let pending: Ref<u32> = Ref::Pending(key.clone());
        assert_eq!(pending.pending_key(), Some(&key));

        let broken: Ref<u32> = Ref::Broken(BreakCause::NotFound { what: key });
        assert_eq!(broken.pending_key(), None);
        assert!(broken.break_cause().is_some());
    }

    #[test]
    fn scoped_name_renders_idl_syntax() {
        let name = ScopedName::absolute(vec!["Robot".to_string(), "Pose".to_string()]);
        assert_eq!(name.to_string(), "::Robot::Pose");
        let relative = ScopedName::relative(vec!["Pose".to_string()]);
        assert_eq!(relative.to_string(), "Pose");
        assert!(name.is_absolute());
        assert!(!relative.is_absolute());
    }

    #[test]
    fn scoped_names_order_segment_wise_for_deterministic_indices() {
        let a = ScopedName::absolute(vec!["A".to_string()]);
        let ab = ScopedName::absolute(vec!["A".to_string(), "B".to_string()]);
        let b = ScopedName::absolute(vec!["B".to_string()]);
        let mut all = vec![b.clone(), ab.clone(), a.clone()];
        all.sort();
        assert_eq!(all, vec![a, ab, b]);
    }
}
