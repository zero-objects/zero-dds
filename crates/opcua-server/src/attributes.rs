// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Part-3 Node attribute model the Attribute Service Set operates on:
//! the attribute ids (Part 6 Annex A.1), the per-NodeClass attribute sets
//! (Part 3 §5), the `AttributeWriteMask` bits (Part 3 §8.60), the
//! `AccessLevelType` bits (Part 3 §8.57) and the `NumericRange` index range
//! (Part 4 §7.27 / Annex A.3).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{ExtensionObject, Variant, VariantValue};
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_gateway::types::LocalizedText;

use crate::status;

/// Attribute identifiers (Part 6 Annex A.1, Table A.1).
pub mod attribute_ids {
    /// `NodeId`.
    pub const NODE_ID: u32 = 1;
    /// `NodeClass`.
    pub const NODE_CLASS: u32 = 2;
    /// `BrowseName`.
    pub const BROWSE_NAME: u32 = 3;
    /// `DisplayName`.
    pub const DISPLAY_NAME: u32 = 4;
    /// `Description`.
    pub const DESCRIPTION: u32 = 5;
    /// `WriteMask`.
    pub const WRITE_MASK: u32 = 6;
    /// `UserWriteMask`.
    pub const USER_WRITE_MASK: u32 = 7;
    /// `IsAbstract`.
    pub const IS_ABSTRACT: u32 = 8;
    /// `Symmetric`.
    pub const SYMMETRIC: u32 = 9;
    /// `InverseName`.
    pub const INVERSE_NAME: u32 = 10;
    /// `ContainsNoLoops`.
    pub const CONTAINS_NO_LOOPS: u32 = 11;
    /// `EventNotifier`.
    pub const EVENT_NOTIFIER: u32 = 12;
    /// `Value`.
    pub const VALUE: u32 = 13;
    /// `DataType`.
    pub const DATA_TYPE: u32 = 14;
    /// `ValueRank`.
    pub const VALUE_RANK: u32 = 15;
    /// `ArrayDimensions`.
    pub const ARRAY_DIMENSIONS: u32 = 16;
    /// `AccessLevel`.
    pub const ACCESS_LEVEL: u32 = 17;
    /// `UserAccessLevel`.
    pub const USER_ACCESS_LEVEL: u32 = 18;
    /// `MinimumSamplingInterval`.
    pub const MINIMUM_SAMPLING_INTERVAL: u32 = 19;
    /// `Historizing`.
    pub const HISTORIZING: u32 = 20;
    /// `Executable`.
    pub const EXECUTABLE: u32 = 21;
    /// `UserExecutable`.
    pub const USER_EXECUTABLE: u32 = 22;
    /// `DataTypeDefinition`.
    pub const DATA_TYPE_DEFINITION: u32 = 23;
    /// `RolePermissions`.
    pub const ROLE_PERMISSIONS: u32 = 24;
    /// `UserRolePermissions`.
    pub const USER_ROLE_PERMISSIONS: u32 = 25;
    /// `AccessRestrictions`.
    pub const ACCESS_RESTRICTIONS: u32 = 26;
    /// `AccessLevelEx`.
    pub const ACCESS_LEVEL_EX: u32 = 27;
    /// The highest defined attribute id; anything above is structurally
    /// invalid rather than merely unsupported.
    pub const MAX: u32 = ACCESS_LEVEL_EX;
}

/// `AttributeWriteMask` bits (Part 3 §8.60, Table 44).
pub mod write_mask {
    /// `AccessLevel` writeable.
    pub const ACCESS_LEVEL: u32 = 1 << 0;
    /// `ArrayDimensions` writeable.
    pub const ARRAY_DIMENSIONS: u32 = 1 << 1;
    /// `BrowseName` writeable.
    pub const BROWSE_NAME: u32 = 1 << 2;
    /// `ContainsNoLoops` writeable.
    pub const CONTAINS_NO_LOOPS: u32 = 1 << 3;
    /// `DataType` writeable.
    pub const DATA_TYPE: u32 = 1 << 4;
    /// `Description` writeable.
    pub const DESCRIPTION: u32 = 1 << 5;
    /// `DisplayName` writeable.
    pub const DISPLAY_NAME: u32 = 1 << 6;
    /// `EventNotifier` writeable.
    pub const EVENT_NOTIFIER: u32 = 1 << 7;
    /// `Executable` writeable.
    pub const EXECUTABLE: u32 = 1 << 8;
    /// `Historizing` writeable.
    pub const HISTORIZING: u32 = 1 << 9;
    /// `InverseName` writeable.
    pub const INVERSE_NAME: u32 = 1 << 10;
    /// `IsAbstract` writeable.
    pub const IS_ABSTRACT: u32 = 1 << 11;
    /// `MinimumSamplingInterval` writeable.
    pub const MINIMUM_SAMPLING_INTERVAL: u32 = 1 << 12;
    /// `NodeClass` writeable.
    pub const NODE_CLASS: u32 = 1 << 13;
    /// `NodeId` writeable.
    pub const NODE_ID: u32 = 1 << 14;
    /// `Symmetric` writeable.
    pub const SYMMETRIC: u32 = 1 << 15;
    /// `UserAccessLevel` writeable.
    pub const USER_ACCESS_LEVEL: u32 = 1 << 16;
    /// `UserExecutable` writeable.
    pub const USER_EXECUTABLE: u32 = 1 << 17;
    /// `UserWriteMask` writeable.
    pub const USER_WRITE_MASK: u32 = 1 << 18;
    /// `ValueRank` writeable.
    pub const VALUE_RANK: u32 = 1 << 19;
    /// `WriteMask` writeable.
    pub const WRITE_MASK: u32 = 1 << 20;
    /// `Value` of a **VariableType** writeable (Part 3 §8.60: for Variables
    /// this bit shall be 0 — their Value is governed by `AccessLevel`).
    pub const VALUE_FOR_VARIABLE_TYPE: u32 = 1 << 21;
    /// `DataTypeDefinition` writeable.
    pub const DATA_TYPE_DEFINITION: u32 = 1 << 22;
    /// `RolePermissions` writeable.
    pub const ROLE_PERMISSIONS: u32 = 1 << 23;
    /// `AccessRestrictions` writeable.
    pub const ACCESS_RESTRICTIONS: u32 = 1 << 24;
    /// `AccessLevelEx` writeable.
    pub const ACCESS_LEVEL_EX: u32 = 1 << 25;
}

/// `AccessLevelType` bits (Part 3 §8.57) — the Variable `Value` access gate.
pub mod access_level {
    /// The current value is readable.
    pub const CURRENT_READ: u8 = 1 << 0;
    /// The current value is writeable.
    pub const CURRENT_WRITE: u8 = 1 << 1;
    /// History is readable.
    pub const HISTORY_READ: u8 = 1 << 2;
    /// History is writeable.
    pub const HISTORY_WRITE: u8 = 1 << 3;
    /// Semantic-change events are produced.
    pub const SEMANTIC_CHANGE: u8 = 1 << 4;
    /// The `StatusCode` of the value is writeable.
    pub const STATUS_WRITE: u8 = 1 << 5;
    /// The timestamps of the value are writeable.
    pub const TIMESTAMP_WRITE: u8 = 1 << 6;
}

/// The `WriteMask` bit that gates writing `attribute_id`, or `None` for the
/// Variable `Value` attribute, which `AccessLevel` gates instead.
#[must_use]
pub const fn write_mask_bit(attribute_id: u32) -> Option<u32> {
    use attribute_ids as a;
    Some(match attribute_id {
        a::NODE_ID => write_mask::NODE_ID,
        a::NODE_CLASS => write_mask::NODE_CLASS,
        a::BROWSE_NAME => write_mask::BROWSE_NAME,
        a::DISPLAY_NAME => write_mask::DISPLAY_NAME,
        a::DESCRIPTION => write_mask::DESCRIPTION,
        a::WRITE_MASK => write_mask::WRITE_MASK,
        a::USER_WRITE_MASK => write_mask::USER_WRITE_MASK,
        a::IS_ABSTRACT => write_mask::IS_ABSTRACT,
        a::SYMMETRIC => write_mask::SYMMETRIC,
        a::INVERSE_NAME => write_mask::INVERSE_NAME,
        a::CONTAINS_NO_LOOPS => write_mask::CONTAINS_NO_LOOPS,
        a::EVENT_NOTIFIER => write_mask::EVENT_NOTIFIER,
        a::DATA_TYPE => write_mask::DATA_TYPE,
        a::VALUE_RANK => write_mask::VALUE_RANK,
        a::ARRAY_DIMENSIONS => write_mask::ARRAY_DIMENSIONS,
        a::ACCESS_LEVEL => write_mask::ACCESS_LEVEL,
        a::USER_ACCESS_LEVEL => write_mask::USER_ACCESS_LEVEL,
        a::MINIMUM_SAMPLING_INTERVAL => write_mask::MINIMUM_SAMPLING_INTERVAL,
        a::HISTORIZING => write_mask::HISTORIZING,
        a::EXECUTABLE => write_mask::EXECUTABLE,
        a::USER_EXECUTABLE => write_mask::USER_EXECUTABLE,
        a::DATA_TYPE_DEFINITION => write_mask::DATA_TYPE_DEFINITION,
        a::ROLE_PERMISSIONS => write_mask::ROLE_PERMISSIONS,
        a::USER_ROLE_PERMISSIONS => write_mask::ROLE_PERMISSIONS,
        a::ACCESS_RESTRICTIONS => write_mask::ACCESS_RESTRICTIONS,
        a::ACCESS_LEVEL_EX => write_mask::ACCESS_LEVEL_EX,
        // Part 3 §8.60: the Value of a Variable is not covered by the
        // WriteMask. VariableType Values use VALUE_FOR_VARIABLE_TYPE, which
        // the caller applies because only it knows the NodeClass.
        _ => return None,
    })
}

/// The Part-3 §5.6 `Variable` attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableAttributes {
    /// `DataType` — the NodeId of the value's DataType Node.
    pub data_type: NodeId,
    /// `ValueRank` (−3 ScalarOrOneDimension, −2 Any, −1 Scalar, 0 OneOrMore,
    /// ≥1 the exact number of dimensions).
    pub value_rank: i32,
    /// `ArrayDimensions` (empty when the rank does not fix them).
    pub array_dimensions: Vec<u32>,
    /// `AccessLevel` bits — see [`access_level`].
    pub access_level: u8,
    /// `UserAccessLevel` bits (never wider than `access_level`).
    pub user_access_level: u8,
    /// `MinimumSamplingInterval` in milliseconds (`0.0` = continuous).
    pub minimum_sampling_interval: f64,
    /// `Historizing`.
    pub historizing: bool,
    /// `AccessLevelEx` — the 32-bit extension of `AccessLevel`.
    pub access_level_ex: u32,
}

impl Default for VariableAttributes {
    fn default() -> Self {
        Self {
            // `BaseDataType` (ns0 i=24) — the least specific DataType.
            data_type: NodeId::numeric(0, 24),
            value_rank: -1, // Scalar
            array_dimensions: Vec::new(),
            access_level: access_level::CURRENT_READ | access_level::CURRENT_WRITE,
            user_access_level: access_level::CURRENT_READ | access_level::CURRENT_WRITE,
            minimum_sampling_interval: 0.0,
            historizing: false,
            access_level_ex: u32::from(access_level::CURRENT_READ | access_level::CURRENT_WRITE),
        }
    }
}

/// The Part-3 §5.7 `VariableType` attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableTypeAttributes {
    /// `IsAbstract`.
    pub is_abstract: bool,
    /// `DataType`.
    pub data_type: NodeId,
    /// `ValueRank`.
    pub value_rank: i32,
    /// `ArrayDimensions`.
    pub array_dimensions: Vec<u32>,
}

impl Default for VariableTypeAttributes {
    fn default() -> Self {
        Self {
            is_abstract: false,
            data_type: NodeId::numeric(0, 24),
            value_rank: -1,
            array_dimensions: Vec::new(),
        }
    }
}

/// The Part-3 §5.8 `Method` attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodAttributes {
    /// `Executable`.
    pub executable: bool,
    /// `UserExecutable`.
    pub user_executable: bool,
}

impl Default for MethodAttributes {
    fn default() -> Self {
        Self {
            executable: true,
            user_executable: true,
        }
    }
}

/// The Part-3 §5.3 `ReferenceType` attributes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReferenceTypeAttributes {
    /// `IsAbstract`.
    pub is_abstract: bool,
    /// `Symmetric`.
    pub symmetric: bool,
    /// `InverseName` (absent for symmetric reference types).
    pub inverse_name: Option<LocalizedText>,
}

/// The Part-3 §5.8.3 `DataType` attributes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataTypeAttributes {
    /// `IsAbstract`.
    pub is_abstract: bool,
    /// `DataTypeDefinition` (a `StructureDefinition`/`EnumDefinition`
    /// ExtensionObject; absent for non-structured DataTypes).
    pub data_type_definition: Option<ExtensionObject>,
}

/// The Part-3 §5.5 `View` attributes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewAttributes {
    /// `ContainsNoLoops`.
    pub contains_no_loops: bool,
    /// `EventNotifier`.
    pub event_notifier: u8,
}

/// The NodeClass-specific attributes of a Node (Part 3 §5). Attributes not
/// carried by the variant a Node holds are answered with
/// `Bad_AttributeIdInvalid`, as Part 4 §5.11.2.4 requires.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassAttributes {
    /// `Object` — `EventNotifier`.
    Object {
        /// `EventNotifier` bits.
        event_notifier: u8,
    },
    /// `Variable`.
    Variable(VariableAttributes),
    /// `Method`.
    Method(MethodAttributes),
    /// `ObjectType` — `IsAbstract`.
    ObjectType {
        /// `IsAbstract`.
        is_abstract: bool,
    },
    /// `VariableType`.
    VariableType(VariableTypeAttributes),
    /// `ReferenceType`.
    ReferenceType(ReferenceTypeAttributes),
    /// `DataType`.
    DataType(DataTypeAttributes),
    /// `View`.
    View(ViewAttributes),
}

// ---------------------------------------------------------------------------
// NumericRange (Part 4 §7.27, BNF in Annex A.3).
// ---------------------------------------------------------------------------

/// One dimension of a [`NumericRange`]: `lo..=hi`, with `lo == hi` for the
/// single-index form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeDimension {
    /// Lower bound (inclusive).
    pub lo: u32,
    /// Upper bound (inclusive).
    pub hi: u32,
}

/// A parsed `NumericRange` — one dimension per `ArrayDimensions` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericRange {
    /// The per-dimension bounds, in `ArrayDimensions` order.
    pub dimensions: Vec<RangeDimension>,
}

impl NumericRange {
    /// Parses the Part 4 Annex A.3 grammar. `None` for the null/empty string
    /// (Part 4 §7.27: "A null or empty string indicates that this parameter is
    /// not used").
    ///
    /// # Errors
    /// [`status::BAD_INDEX_RANGE_INVALID`] on any syntax violation: a
    /// non-digit, a missing bound, `lo >= hi` in the range form (§7.27: "The
    /// first integer shall always have a lower value than the second"), or an
    /// index that does not fit a `u32`.
    pub fn parse(text: &str) -> Result<Option<Self>, u32> {
        if text.is_empty() {
            return Ok(None);
        }
        let mut dimensions = Vec::new();
        for part in text.split(',') {
            let mut bounds = part.split(':');
            let lo = parse_index(bounds.next().unwrap_or(""))?;
            let dim = match bounds.next() {
                None => RangeDimension { lo, hi: lo },
                Some(hi_text) => {
                    let hi = parse_index(hi_text)?;
                    if lo >= hi {
                        return Err(status::BAD_INDEX_RANGE_INVALID);
                    }
                    RangeDimension { lo, hi }
                }
            };
            if bounds.next().is_some() {
                return Err(status::BAD_INDEX_RANGE_INVALID);
            }
            dimensions.push(dim);
        }
        Ok(Some(Self { dimensions }))
    }

    /// The number of elements the range selects, given that every upper bound
    /// has already been clamped to the value's extent.
    #[must_use]
    pub fn len(&self) -> usize {
        self.dimensions
            .iter()
            .map(|d| (d.hi - d.lo) as usize + 1)
            .product()
    }

    /// `true` if the range selects nothing (only possible for an empty
    /// dimension list, which [`parse`](Self::parse) never produces).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dimensions.is_empty()
    }

    /// Applies the range to a Variant, returning the selected elements
    /// (Part 4 §7.27).
    ///
    /// Only single-dimension ranges are applicable to this AddressSpace's
    /// value model, whose `Variant::value` is a flat element sequence; a
    /// multi-dimension range against a value that does not declare that many
    /// `ArrayDimensions` is `Bad_IndexRangeNoData` per §7.27 ("All dimensions
    /// shall be specified for a NumericRange to be valid").
    ///
    /// # Errors
    /// [`status::BAD_INDEX_RANGE_NO_DATA`] if the lower bound is past the end
    /// of the value or the dimension count does not match the value's.
    pub fn apply(&self, value: &Variant) -> Result<Variant, u32> {
        let dim = self.single_dimension(value)?;
        let len = value.value.len();
        if dim.lo as usize >= len {
            // §7.27: "When reading a value and any of the lower bounds of the
            // indexes is out of range the Server shall return a
            // Bad_IndexRangeNoData."
            return Err(status::BAD_INDEX_RANGE_NO_DATA);
        }
        // §7.27: "If any of the upper bounds of the indexes is out of range,
        // the Server shall return partial results."
        let hi = (dim.hi as usize).min(len - 1);
        let selected: Vec<VariantValue> = value.value[dim.lo as usize..=hi].to_vec();
        let count = u32::try_from(selected.len()).unwrap_or(u32::MAX);
        Ok(Variant {
            array_dimensions: if value.array_dimensions.is_empty() {
                Vec::new()
            } else {
                alloc::vec![count]
            },
            value: selected,
        })
    }

    /// Replaces the selected elements of `target` with `source` (the Write
    /// counterpart of [`apply`](Self::apply)).
    ///
    /// # Errors
    /// [`status::BAD_INDEX_RANGE_NO_DATA`] if the range does not lie inside
    /// the target, [`status::BAD_INDEX_RANGE_INVALID`] if the source element
    /// count does not match the range width (Part 4 §5.11.4.4:
    /// `Bad_IndexRangeDataMismatch` is folded into the invalid-range code the
    /// same way `Bad_IndexRangeInvalid` covers a range that cannot be
    /// written).
    pub fn replace(&self, target: &mut Variant, source: &Variant) -> Result<(), u32> {
        let dim = self.single_dimension(target)?;
        let len = target.value.len();
        if dim.lo as usize >= len || dim.hi as usize >= len {
            return Err(status::BAD_INDEX_RANGE_NO_DATA);
        }
        let width = (dim.hi - dim.lo) as usize + 1;
        if source.value.len() != width {
            return Err(status::BAD_INDEX_RANGE_INVALID);
        }
        target.value[dim.lo as usize..=dim.hi as usize].clone_from_slice(&source.value);
        Ok(())
    }

    /// The single dimension this range selects, or `Bad_IndexRangeNoData` for
    /// a multi-dimensional range: `Variant::value` is a flat element sequence,
    /// so no data exists at a `[i,j]` address.
    fn single_dimension(&self, _value: &Variant) -> Result<RangeDimension, u32> {
        match self.dimensions.as_slice() {
            [only] => Ok(*only),
            _ => Err(status::BAD_INDEX_RANGE_NO_DATA),
        }
    }
}

fn parse_index(text: &str) -> Result<u32, u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(status::BAD_INDEX_RANGE_INVALID);
    }
    text.parse::<u32>()
        .map_err(|_| status::BAD_INDEX_RANGE_INVALID)
}

/// `TimestampsToReturn` (Part 4 §7.39, Table 180).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampsToReturn {
    /// `SOURCE` (0).
    Source,
    /// `SERVER` (1).
    Server,
    /// `BOTH` (2).
    Both,
    /// `NEITHER` (3).
    Neither,
}

impl TimestampsToReturn {
    /// Decodes the wire enumeration.
    ///
    /// # Errors
    /// [`status::BAD_TIMESTAMPS_TO_RETURN_INVALID`] for `INVALID` (4) and
    /// every value outside the table.
    pub const fn from_i32(v: i32) -> Result<Self, u32> {
        Ok(match v {
            0 => Self::Source,
            1 => Self::Server,
            2 => Self::Both,
            3 => Self::Neither,
            _ => return Err(status::BAD_TIMESTAMPS_TO_RETURN_INVALID),
        })
    }

    /// Strips the timestamps this setting does not ask for.
    pub fn apply(self, dv: &mut zerodds_opcua_gateway::data_value::DataValue) {
        let (source, server) = match self {
            Self::Source => (true, false),
            Self::Server => (false, true),
            Self::Both => (true, true),
            Self::Neither => (false, false),
        };
        if !source {
            dv.source_timestamp = None;
            dv.source_pico_sec = None;
        }
        if !server {
            dv.server_timestamp = None;
            dv.server_pico_sec = None;
        }
    }
}

/// `MonitoringMode` (Part 4 §7.23).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitoringMode {
    /// `DISABLED` (0) — the item is neither sampled nor reported.
    Disabled,
    /// `SAMPLING` (1) — sampled and queued, but not reported.
    Sampling,
    /// `REPORTING` (2) — sampled, queued and reported.
    Reporting,
}

impl MonitoringMode {
    /// Decodes the wire enumeration.
    ///
    /// # Errors
    /// [`status::BAD_MONITORING_MODE_INVALID`] outside `0..=2`.
    pub const fn from_i32(v: i32) -> Result<Self, u32> {
        Ok(match v {
            0 => Self::Disabled,
            1 => Self::Sampling,
            2 => Self::Reporting,
            _ => return Err(status::BAD_MONITORING_MODE_INVALID),
        })
    }

    /// The wire value.
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        match self {
            Self::Disabled => 0,
            Self::Sampling => 1,
            Self::Reporting => 2,
        }
    }
}

/// A `LocalizedText` carrying just `text` in the invariant locale.
#[must_use]
pub fn text(value: &str) -> LocalizedText {
    LocalizedText {
        locale: None,
        text: Some(String::from(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn array(values: &[i32]) -> Variant {
        Variant {
            array_dimensions: alloc::vec![values.len() as u32],
            value: values.iter().map(|v| VariantValue::Int32(*v)).collect(),
        }
    }

    #[test]
    fn numeric_range_parses_the_annex_a3_grammar() {
        assert_eq!(NumericRange::parse("").expect("empty"), None);
        assert_eq!(
            NumericRange::parse("6").expect("single").expect("some"),
            NumericRange {
                dimensions: alloc::vec![RangeDimension { lo: 6, hi: 6 }],
            }
        );
        assert_eq!(
            NumericRange::parse("5:7").expect("range").expect("some"),
            NumericRange {
                dimensions: alloc::vec![RangeDimension { lo: 5, hi: 7 }],
            }
        );
        assert_eq!(
            NumericRange::parse("1:2,0:1")
                .expect("multi")
                .expect("some")
                .dimensions
                .len(),
            2
        );
    }

    #[test]
    fn numeric_range_rejects_the_forms_the_spec_calls_invalid() {
        // §7.27: "'6 ,0' and '3,2' are not [valid]" for the single-integer
        // construct — whitespace is never permitted, and "7:5" / "5:5" break
        // the lo < hi rule.
        for bad in ["6 ,0", "7:5", "5:5", "a", "1:", ":2", "1:2:3", "1,,2", "-1"] {
            assert_eq!(
                NumericRange::parse(bad),
                Err(status::BAD_INDEX_RANGE_INVALID),
                "expected {bad:?} to be rejected"
            );
        }
    }

    #[test]
    fn numeric_range_apply_selects_and_clamps() {
        let v = array(&[10, 11, 12, 13]);
        let one = NumericRange::parse("1").expect("p").expect("s");
        assert_eq!(one.apply(&v).expect("apply").value.len(), 1);
        assert_eq!(
            one.apply(&v).expect("apply").value[0],
            VariantValue::Int32(11)
        );
        // Upper bound past the end yields partial results (§7.27).
        let wide = NumericRange::parse("2:9").expect("p").expect("s");
        assert_eq!(wide.apply(&v).expect("apply").value.len(), 2);
        // Lower bound past the end is Bad_IndexRangeNoData (§7.27).
        let past = NumericRange::parse("9:11").expect("p").expect("s");
        assert_eq!(past.apply(&v), Err(status::BAD_INDEX_RANGE_NO_DATA));
    }

    #[test]
    fn numeric_range_replace_writes_in_place() {
        let mut v = array(&[10, 11, 12, 13]);
        let r = NumericRange::parse("1:2").expect("p").expect("s");
        r.replace(&mut v, &array(&[21, 22])).expect("replace");
        assert_eq!(
            v.value,
            alloc::vec![
                VariantValue::Int32(10),
                VariantValue::Int32(21),
                VariantValue::Int32(22),
                VariantValue::Int32(13),
            ]
        );
        // A source whose element count does not match the range is rejected.
        assert_eq!(
            r.replace(&mut v, &array(&[1])),
            Err(status::BAD_INDEX_RANGE_INVALID)
        );
    }

    #[test]
    fn write_mask_bit_matches_table_44() {
        use attribute_ids as a;
        assert_eq!(write_mask_bit(a::ACCESS_LEVEL), Some(1 << 0));
        assert_eq!(write_mask_bit(a::BROWSE_NAME), Some(1 << 2));
        assert_eq!(write_mask_bit(a::WRITE_MASK), Some(1 << 20));
        assert_eq!(write_mask_bit(a::ACCESS_LEVEL_EX), Some(1 << 25));
        // Part 3 §8.60: the Value attribute is not in the mask.
        assert_eq!(write_mask_bit(a::VALUE), None);
    }

    #[test]
    fn timestamps_to_return_strips_what_was_not_asked_for() {
        use zerodds_opcua_gateway::data_value::DataValue;
        let base = DataValue {
            value: None,
            status: Some(0),
            source_timestamp: Some(1),
            server_timestamp: Some(2),
            source_pico_sec: Some(3),
            server_pico_sec: Some(4),
        };
        let mut source = base.clone();
        TimestampsToReturn::Source.apply(&mut source);
        assert_eq!(
            (source.source_timestamp, source.server_timestamp),
            (Some(1), None)
        );
        let mut server = base.clone();
        TimestampsToReturn::Server.apply(&mut server);
        assert_eq!(
            (server.source_timestamp, server.server_timestamp),
            (None, Some(2))
        );
        let mut neither = base.clone();
        TimestampsToReturn::Neither.apply(&mut neither);
        assert_eq!(
            (neither.source_timestamp, neither.server_timestamp),
            (None, None)
        );
        let mut both = base;
        TimestampsToReturn::Both.apply(&mut both);
        assert_eq!(
            (both.source_timestamp, both.server_timestamp),
            (Some(1), Some(2))
        );
        assert_eq!(
            TimestampsToReturn::from_i32(4),
            Err(status::BAD_TIMESTAMPS_TO_RETURN_INVALID)
        );
    }
}
