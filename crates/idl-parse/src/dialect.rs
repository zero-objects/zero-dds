// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Which dialect the parser accepts. Spec §3.5.
//!
//! Strict OMG IDL 4.2 by default. A vendor flag admits that vendor's
//! extensions, and a construct admitted only by a flag records which
//! one — so a later validation against `Profile::Idl42Core` can say
//! that the file is not portable and why.

/// The set of dialects a parse accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Dialect {
    /// RTI Connext extensions: `keylist`.
    pub rti: bool,
    /// Cyclone DDS extensions.
    pub cyclone: bool,
    /// eProsima Fast DDS extensions.
    pub fast_dds: bool,
    /// OpenDDS extensions.
    pub open_dds: bool,
    /// ROS 2 dialect: admits unqualified member types that differ only in case
    /// from the member name (e.g., `Pose pose;`).
    pub ros2: bool,
    /// Identifiers are compared with regard to case.
    ///
    /// OMG IDL 4.2 §7.2.3: "Upper- and lower-case letters are treated as
    /// the same letter" when two identifiers are compared for a
    /// collision, and §7.2.4 has an identifier collide with a keyword it
    /// differs from only in case. With this set, both comparisons are
    /// exact: `module shape` and `struct Shape` stand in one scope, and
    /// `Struct` is an identifier. It is what `idlc -f case-sensitive` of
    /// Cyclone DDS does. A file that needs it is not OMG IDL, and no
    /// vendor flag turns it on.
    pub case_sensitive: bool,
    /// The file is OMG CIDL: CORBA 3.3 Part 3 §8.
    ///
    /// CIDL is IDL plus the PSDL declarations of Persistent State
    /// Service 2.0 §3.2 (`abstract storagetype`, `abstract storagehome`,
    /// `storagetype`, `storagehome`) plus the `composition`. §8.2.1
    /// reserves the words of Table 8.2 and of PSDL's Table 3-2 as
    /// keywords; with this set, an identifier spelled exactly like one of
    /// them ([`zerodds_idl_cst::CIDL_KEYWORDS`]) is that keyword, and
    /// only the escaped form `_state` is the identifier `state`. Off, an
    /// IDL file keeps these words as identifiers. A `.cdl` file is read
    /// with it on (§8.1). Not a vendor dialect: it is OMG's own language.
    pub cidl: bool,
}

impl Dialect {
    /// OMG IDL 4.2 and nothing else.
    ///
    /// The default, because a file that parses under it is one every
    /// other vendor's compiler also reads.
    pub const STRICT: Self = Self {
        rti: false,
        cyclone: false,
        fast_dds: false,
        open_dds: false,
        ros2: false,
        case_sensitive: false,
        cidl: false,
    };

    /// Every vendor dialect at once.
    pub const ALL_VENDORS: Self = Self {
        rti: true,
        cyclone: true,
        fast_dds: true,
        open_dds: true,
        ros2: false,
        case_sensitive: false,
        cidl: false,
    };

    /// Whether any vendor extension is admitted.
    #[must_use]
    pub const fn any_vendor(self) -> bool {
        self.rti || self.cyclone || self.fast_dds || self.open_dds || self.ros2
    }

    /// The name of the flag that admits a construct, for a diagnostic
    /// that has to say what made a file unportable.
    #[must_use]
    pub const fn flag_name(admitted_by: VendorFlag) -> &'static str {
        match admitted_by {
            VendorFlag::Rti => "rti",
            VendorFlag::Cyclone => "cyclone",
            VendorFlag::FastDds => "fast_dds",
            VendorFlag::OpenDds => "open_dds",
            VendorFlag::Ros2 => "ros2",
        }
    }

    /// Whether this dialect admits `flag`.
    #[must_use]
    pub const fn admits(self, flag: VendorFlag) -> bool {
        match flag {
            VendorFlag::Rti => self.rti,
            VendorFlag::Cyclone => self.cyclone,
            VendorFlag::FastDds => self.fast_dds,
            VendorFlag::OpenDds => self.open_dds,
            VendorFlag::Ros2 => self.ros2,
        }
    }
}

/// One vendor flag, for naming which admitted a construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VendorFlag {
    /// RTI Connext.
    Rti,
    /// Cyclone DDS.
    Cyclone,
    /// Fast DDS.
    FastDds,
    /// OpenDDS.
    OpenDds,
    /// ROS 2 dialect.
    Ros2,
}

impl VendorFlag {
    /// Every flag, so a test can assert the set is complete.
    pub const ALL: [Self; 5] = [
        Self::Rti,
        Self::Cyclone,
        Self::FastDds,
        Self::OpenDds,
        Self::Ros2,
    ];
}
