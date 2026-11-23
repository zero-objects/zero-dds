// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA system exceptions (CORBA 3.3 Part 1 §8.12): the identity of
//! the exception, its minor code and its completion status, and the
//! CDR form a GIOP `SYSTEM_EXCEPTION` reply carries (Part 2 §9.4.3.2:
//! `string exception_id; unsigned long minor_code_value; unsigned long
//! completion_status;`).
//!
//! A minor code is a 20-bit vendor minor codeset id (VMCID) in the
//! high-order bits and a 12-bit code in the low-order bits. The
//! standard minor codes of Annex A.6 are or-ed with [`OMGVMCID`];
//! [`SystemException::omg`] does that.
//!
//! A system exception whose repository id this ORB does not know
//! decodes as `UNKNOWN` with standard minor code 2 and the completion
//! status preserved (§8.12.2).

use alloc::string::String;
use core::fmt;

use zerodds_wire::{Reader, WireError, Writer};

/// `CORBA::OMGVMCID` (§8.12.2): the VMCID the OMG holds, in the
/// high-order 20 bits of a minor code.
pub const OMGVMCID: u32 = 0x4f4d_0000;

/// The mask of the VMCID in a minor code (the high-order 20 bits).
pub const VMCID_MASK: u32 = 0xffff_f000;

/// `CORBA::completion_status` (§8.12.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CompletionStatus {
    /// `COMPLETED_YES` (0): the object implementation completed
    /// processing before the exception was raised.
    Yes,
    /// `COMPLETED_NO` (1): the object implementation was never
    /// initiated.
    No,
    /// `COMPLETED_MAYBE` (2): completion is indeterminate.
    Maybe,
}

impl CompletionStatus {
    /// The enum's wire value.
    #[must_use]
    pub const fn to_u32(self) -> u32 {
        match self {
            Self::Yes => 0,
            Self::No => 1,
            Self::Maybe => 2,
        }
    }

    /// The status for a wire value; `None` for a value outside the enum.
    #[must_use]
    pub const fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Yes),
            1 => Some(Self::No),
            2 => Some(Self::Maybe),
            _ => None,
        }
    }

    /// The IDL enumerator name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Yes => "COMPLETED_YES",
            Self::No => "COMPLETED_NO",
            Self::Maybe => "COMPLETED_MAYBE",
        }
    }
}

macro_rules! system_exception_kinds {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        /// The standard system exceptions of §8.12.3, one variant per
        /// `exception <NAME> ex_body;` of the PIDL.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum SystemExceptionKind {
            $(
                #[doc = concat!("`CORBA::", $name, "`.")]
                $variant,
            )+
        }

        impl SystemExceptionKind {
            /// Every standard system exception, in the order of §8.12.3.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The IDL name, e.g. `BAD_PARAM`.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }

            /// The repository id, e.g. `IDL:omg.org/CORBA/BAD_PARAM:1.0`.
            #[must_use]
            pub const fn repository_id(self) -> &'static str {
                match self {
                    $(Self::$variant => concat!("IDL:omg.org/CORBA/", $name, ":1.0"),)+
                }
            }

            /// The kind a repository id names; `None` for an id that is
            /// not one of the standard system exceptions.
            #[must_use]
            pub fn from_repository_id(id: &str) -> Option<Self> {
                match id {
                    $(concat!("IDL:omg.org/CORBA/", $name, ":1.0") => Some(Self::$variant),)+
                    _ => None,
                }
            }

            /// The kind an IDL name names (`BAD_PARAM`).
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

system_exception_kinds! {
    Unknown => "UNKNOWN",
    BadParam => "BAD_PARAM",
    NoMemory => "NO_MEMORY",
    ImpLimit => "IMP_LIMIT",
    CommFailure => "COMM_FAILURE",
    InvObjref => "INV_OBJREF",
    NoPermission => "NO_PERMISSION",
    Internal => "INTERNAL",
    Marshal => "MARSHAL",
    Initialize => "INITIALIZE",
    NoImplement => "NO_IMPLEMENT",
    BadTypecode => "BAD_TYPECODE",
    BadOperation => "BAD_OPERATION",
    NoResources => "NO_RESOURCES",
    NoResponse => "NO_RESPONSE",
    PersistStore => "PERSIST_STORE",
    BadInvOrder => "BAD_INV_ORDER",
    Transient => "TRANSIENT",
    FreeMem => "FREE_MEM",
    InvIdent => "INV_IDENT",
    InvFlag => "INV_FLAG",
    IntfRepos => "INTF_REPOS",
    BadContext => "BAD_CONTEXT",
    ObjAdapter => "OBJ_ADAPTER",
    DataConversion => "DATA_CONVERSION",
    ObjectNotExist => "OBJECT_NOT_EXIST",
    TransactionRequired => "TRANSACTION_REQUIRED",
    TransactionRolledback => "TRANSACTION_ROLLEDBACK",
    InvalidTransaction => "INVALID_TRANSACTION",
    InvPolicy => "INV_POLICY",
    CodesetIncompatible => "CODESET_INCOMPATIBLE",
    Rebind => "REBIND",
    Timeout => "TIMEOUT",
    TransactionUnavailable => "TRANSACTION_UNAVAILABLE",
    TransactionMode => "TRANSACTION_MODE",
    BadQos => "BAD_QOS",
    InvalidActivity => "INVALID_ACTIVITY",
    ActivityCompleted => "ACTIVITY_COMPLETED",
    ActivityRequired => "ACTIVITY_REQUIRED",
}

/// A CORBA system exception: kind, minor code and completion status
/// (`ex_body`, §8.12.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SystemException {
    /// Which standard system exception.
    pub kind: SystemExceptionKind,
    /// The minor code: VMCID in the high-order 20 bits, code in the
    /// low-order 12 bits.
    pub minor: u32,
    /// How far the operation got.
    pub completed: CompletionStatus,
}

impl SystemException {
    /// An exception with a minor code as given.
    #[must_use]
    pub const fn new(kind: SystemExceptionKind, minor: u32, completed: CompletionStatus) -> Self {
        Self {
            kind,
            minor,
            completed,
        }
    }

    /// An exception with a standard minor code of Annex A.6 (`code` is
    /// or-ed with [`OMGVMCID`]).
    #[must_use]
    pub const fn omg(kind: SystemExceptionKind, code: u32, completed: CompletionStatus) -> Self {
        Self::new(kind, OMGVMCID | (code & !VMCID_MASK), completed)
    }

    /// The repository id of the exception.
    #[must_use]
    pub const fn repository_id(&self) -> &'static str {
        self.kind.repository_id()
    }

    /// The VMCID of the minor code (high-order 20 bits, low bits zero).
    #[must_use]
    pub const fn vmcid(&self) -> u32 {
        self.minor & VMCID_MASK
    }

    /// The standard minor code (low-order 12 bits) when the minor code
    /// carries the OMG VMCID, else `None`.
    #[must_use]
    pub const fn omg_minor(&self) -> Option<u32> {
        if self.vmcid() == OMGVMCID {
            Some(self.minor & !VMCID_MASK)
        } else {
            None
        }
    }

    /// Writes the body of a GIOP `SYSTEM_EXCEPTION` reply (Part 2
    /// §9.4.3.2): repository id, minor code, completion status.
    pub fn encode(&self, w: &mut Writer) {
        w.write_string(self.repository_id());
        w.write_u32(self.minor);
        w.write_u32(self.completed.to_u32());
    }

    /// Reads the body of a GIOP `SYSTEM_EXCEPTION` reply.
    ///
    /// A repository id that names no standard system exception decodes
    /// as `UNKNOWN` with standard minor code 2 and the completion
    /// status of the wire (§8.12.2).
    ///
    /// # Errors
    /// The wire error of a truncated body, and
    /// [`WireError::OutOfRange`] for a completion status outside the
    /// enum.
    pub fn decode(r: &mut Reader<'_>) -> Result<Self, WireError> {
        let (exc, _) = Self::decode_with_id(r)?;
        Ok(exc)
    }

    /// As [`decode`](Self::decode), and also returns the repository id
    /// as received (for a non-standard system exception it differs from
    /// the `UNKNOWN` id of the result).
    ///
    /// # Errors
    /// As [`decode`](Self::decode).
    pub fn decode_with_id(r: &mut Reader<'_>) -> Result<(Self, String), WireError> {
        let id = r.read_string()?;
        let minor = r.read_u32()?;
        let at = r.position();
        let status = r.read_u32()?;
        let completed =
            CompletionStatus::from_u32(status).ok_or_else(|| WireError::OutOfRange {
                at,
                expected: String::from("completion_status 0, 1 or 2"),
            })?;
        let exc = match SystemExceptionKind::from_repository_id(&id) {
            Some(kind) => Self::new(kind, minor, completed),
            None => Self::omg(SystemExceptionKind::Unknown, 2, completed),
        };
        Ok((exc, id))
    }
}

impl fmt::Display for SystemException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CORBA::{} (minor {:#010x}, {})",
            self.kind.name(),
            self.minor,
            self.completed.name()
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_wire::Endianness;

    #[test]
    fn corba_p1_8_12_3_every_standard_system_exception_has_its_repository_id() {
        assert_eq!(SystemExceptionKind::ALL.len(), 39, "§8.12.3 lists 39");
        for kind in SystemExceptionKind::ALL {
            let id = kind.repository_id();
            assert_eq!(
                id,
                alloc::format!("IDL:omg.org/CORBA/{}:1.0", kind.name()),
                "{kind:?}"
            );
            assert_eq!(SystemExceptionKind::from_repository_id(id), Some(*kind));
            assert_eq!(SystemExceptionKind::from_name(kind.name()), Some(*kind));
        }
        assert_eq!(
            SystemExceptionKind::TransactionRolledback.repository_id(),
            "IDL:omg.org/CORBA/TRANSACTION_ROLLEDBACK:1.0"
        );
        assert_eq!(
            SystemExceptionKind::from_repository_id("IDL:omg.org/CORBA/Object:1.0"),
            None
        );
    }

    #[test]
    fn corba_p1_8_12_2_completion_status_values_follow_the_enum() {
        assert_eq!(CompletionStatus::Yes.to_u32(), 0);
        assert_eq!(CompletionStatus::No.to_u32(), 1);
        assert_eq!(CompletionStatus::Maybe.to_u32(), 2);
        for v in 0..3 {
            assert_eq!(CompletionStatus::from_u32(v).unwrap().to_u32(), v);
        }
        assert_eq!(CompletionStatus::from_u32(3), None);
    }

    #[test]
    fn corba_p1_8_12_2_a_standard_minor_code_carries_the_omg_vmcid() {
        assert_eq!(OMGVMCID, 0x4f4d_0000);
        let e = SystemException::omg(SystemExceptionKind::NoImplement, 1, CompletionStatus::No);
        // Annex A.6: "Missing local value implementation" is 0x4f4d0001.
        assert_eq!(e.minor, 0x4f4d_0001);
        assert_eq!(e.vmcid(), OMGVMCID);
        assert_eq!(e.omg_minor(), Some(1));
        let vendor = SystemException::new(
            SystemExceptionKind::Internal,
            0x5441_0007,
            CompletionStatus::Maybe,
        );
        assert_eq!(vendor.omg_minor(), None);
        assert_eq!(vendor.vmcid(), 0x5441_0000);
    }

    #[test]
    fn corba_p2_9_4_3_system_exception_body_round_trips_in_both_byte_orders() {
        let e = SystemException::omg(SystemExceptionKind::BadParam, 7, CompletionStatus::Maybe);
        for endianness in [Endianness::Big, Endianness::Little] {
            let mut w = Writer::new(endianness);
            e.encode(&mut w);
            let bytes = w.finish();
            let mut r = Reader::new(&bytes, endianness);
            assert_eq!(r.read_string().unwrap(), "IDL:omg.org/CORBA/BAD_PARAM:1.0");
            assert_eq!(r.read_u32().unwrap(), 0x4f4d_0007);
            assert_eq!(r.read_u32().unwrap(), 2);
            let mut r = Reader::new(&bytes, endianness);
            assert_eq!(SystemException::decode(&mut r).unwrap(), e);
            assert_eq!(r.remaining(), 0);
        }
    }

    #[test]
    fn corba_p1_8_12_2_a_non_standard_system_exception_is_presented_as_unknown_minor_2() {
        let mut w = Writer::new(Endianness::Big);
        w.write_string("IDL:vendor.example/CORBA/OWN_FAILURE:1.0");
        w.write_u32(0x1234_5678);
        w.write_u32(0); // COMPLETED_YES
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        let (e, id) = SystemException::decode_with_id(&mut r).unwrap();
        assert_eq!(e.kind, SystemExceptionKind::Unknown);
        assert_eq!(e.minor, OMGVMCID | 2);
        assert_eq!(e.completed, CompletionStatus::Yes, "completion preserved");
        assert_eq!(id, "IDL:vendor.example/CORBA/OWN_FAILURE:1.0");
    }

    #[test]
    fn corba_p2_9_4_3_a_completion_status_outside_the_enum_is_refused() {
        let mut w = Writer::new(Endianness::Big);
        w.write_string("IDL:omg.org/CORBA/MARSHAL:1.0");
        w.write_u32(0);
        w.write_u32(7);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, Endianness::Big);
        assert!(matches!(
            SystemException::decode(&mut r),
            Err(WireError::OutOfRange { .. })
        ));
        let mut r = Reader::new(&bytes[..10], Endianness::Big);
        assert!(SystemException::decode(&mut r).is_err(), "truncated body");
    }

    #[test]
    fn corba_p1_8_12_display_names_kind_minor_and_completion() {
        let e = SystemException::omg(SystemExceptionKind::Transient, 2, CompletionStatus::No);
        assert_eq!(
            alloc::format!("{e}"),
            "CORBA::TRANSIENT (minor 0x4f4d0002, COMPLETED_NO)"
        );
    }
}
