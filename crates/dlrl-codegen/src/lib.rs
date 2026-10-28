// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DLRL code generation — DDS 1.4 Annex B.4.
//!
//! Crate `zerodds-dlrl-codegen`. Safety classification: **STANDARD**.
//!
//! Turns the three DLRL pragmas — `DCPS_DATA_TYPE`, `DCPS_DATA_KEY` and
//! `DCPS_DLRL_RELATION` — into the object and home classes an
//! application writes against. One object class per data type, a getter
//! per key field, a getter and a setter per mono-valued attribute, a
//! collection accessor per multi-valued attribute, and an accessor per
//! relation.
//!
//! ## Why this emits text and not a `lang-cst` tree
//!
//! Everything else in this tree that generates code goes through
//! `zerodds-lang-cst` and `zerodds-lang-render`, because what it
//! generates has to agree with a wire format octet for octet, and a
//! typed tree is what keeps twelve backends from each inventing their
//! own spacing around the same layout decision.
//!
//! A DLRL object class is not on a wire. §B.6 describes the shape an
//! application programs against — a class deriving from `ObjectRoot`
//! with a `RefList` per relation — and no reader ever parses it. There is
//! nothing here for a CST to keep consistent across backends, and the
//! four targets below include two (`C#`, TypeScript) that `lang-cst` has
//! no dialect for. Emitting text is the right level for this and would be
//! the wrong level one layer over.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod cpp;
mod csharp;
mod java;
mod ts;

pub use cpp::{generate_cpp_home, generate_cpp_object};
pub use csharp::{generate_csharp_home, generate_csharp_object};
pub use java::{generate_java_home, generate_java_object};
pub use ts::{generate_ts_class, generate_ts_import, generate_ts_interface};

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_dlrl::pragma::DlrlPragma;

/// The IDL type of a DLRL key or attribute member, which each backend
/// spells in its language's IDL mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlrlFieldType {
    /// `boolean`.
    Boolean,
    /// `char`.
    Char,
    /// `octet`.
    Octet,
    /// `int8`.
    Int8,
    /// `uint8`.
    UInt8,
    /// `short`.
    Short,
    /// `unsigned short`.
    UShort,
    /// `long`.
    Long,
    /// `unsigned long`.
    ULong,
    /// `long long`.
    LongLong,
    /// `unsigned long long`.
    ULongLong,
    /// `float`.
    Float,
    /// `double`.
    Double,
    /// `string`, bounded or not.
    String,
}

/// The type of a key or of a mono-valued attribute. DDS 1.2 §8.1.2.2
/// lists three: a basic type, an enumeration and a simple structure
/// (footnote 3: one that maps inside one DCPS data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DlrlMonoType {
    /// A basic type.
    Basic(DlrlFieldType),
    /// An IDL enumeration.
    Enum {
        /// Scoped name without the leading `::` — `demo::Side`.
        name: String,
        /// The enumerator a new object starts with: the one
        /// `@default_literal` names, else the first (IDL 4.2
        /// §7.3.1.2.1.10). TypeScript needs it to initialise the field.
        default: String,
    },
    /// A simple structure: every member a basic type, an enumeration, a
    /// simple structure or a fixed array of those, so its fields are a
    /// fixed set of DCPS cells (§8.1.4.2.3).
    Struct {
        /// Scoped name without the leading `::` — `demo::Money`.
        name: String,
    },
}

impl DlrlMonoType {
    /// The scoped name of the IDL type this refers to, when it is one the
    /// IDL declares rather than a basic type.
    #[must_use]
    pub fn declared_name(&self) -> Option<&str> {
        match self {
            Self::Basic(_) => None,
            Self::Enum { name, .. } | Self::Struct { name } => Some(name),
        }
    }
}

/// The collection basis of a multi-valued attribute (DDS 1.2 §8.1.2.2):
/// a list, or a map by string or by integer (§8.1.6.3.16–19). IDL has no
/// set, so nothing maps to §8.1.6.3.17's `Set`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DlrlCollection {
    /// A `sequence` or a one-dimensional array: ordered, indexed.
    List,
    /// A `map` keyed by `string`.
    StrMap,
    /// A `map` keyed by an integer type, carried so that no language
    /// narrows a 64-bit key to the PSM's `long`.
    IntMap(DlrlFieldType),
}

/// The type of one attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DlrlAttributeType {
    /// One value, in the main topic (§8.1.4.3).
    Mono(DlrlMonoType),
    /// A collection of homogeneously typed items; §8.1.4.3 gives it a
    /// topic of its own, named `<class>.<attribute>`, keyed by the owner's
    /// oid plus `index`.
    Multi {
        /// List or map.
        basis: DlrlCollection,
        /// The type of each item.
        item: DlrlMonoType,
    },
}

impl DlrlAttributeType {
    /// The value's type, or the type of each item.
    #[must_use]
    pub fn value_type(&self) -> &DlrlMonoType {
        match self {
            Self::Mono(ty) | Self::Multi { item: ty, .. } => ty,
        }
    }
}

/// Every pragma that pertains to one type, gathered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DlrlTypeInfo {
    /// Fully qualified type name — `demo::Trade`.
    pub name: String,
    /// Key fields, in the order the pragmas named them.
    pub keys: Vec<String>,
    /// The IDL type of each key, by key name. The pragmas carry only the
    /// names; the caller fills this from the type model. A key absent here
    /// is written as a type that does not exist in the target language, so
    /// the generated class fails to compile instead of carrying a guessed
    /// type — every key used to be a 32-bit integer whatever the IDL said.
    pub key_types: BTreeMap<String, DlrlMonoType>,
    /// Relations, as `(relation name, target type)`.
    pub relations: Vec<(String, String)>,
    /// The attributes: every member of the data type that is neither a
    /// key nor a relation, in declaration order, with its type. DDS 1.2
    /// §8.1.4.3 places the mono-valued ones in the class's main topic, one
    /// DCPS field each under the attribute's name (`name.sub` per field of
    /// a structure), and gives each multi-valued one a topic of its own;
    /// §8.2.3.5 sets them through the object (`t1->x(1000.0)`). The
    /// pragmas do not name them; the caller fills this from the type model.
    pub attributes: Vec<(String, DlrlAttributeType)>,
}

impl DlrlTypeInfo {
    /// The last segment of the scoped name, which is the class name in
    /// every target below.
    #[must_use]
    pub fn simple_name(&self) -> &str {
        simple_name(&self.name)
    }

    /// The IDL type of `key`, when the caller resolved it.
    #[must_use]
    pub fn key_type(&self, key: &str) -> Option<&DlrlMonoType> {
        self.key_types.get(key)
    }

    /// The scoped names of the IDL enumerations and structures the keys
    /// and attributes name. A class that names one needs the data types
    /// `idlc generate` writes for the same file; one that names none
    /// compiles on its own.
    #[must_use]
    pub fn declared_types(&self) -> BTreeSet<&str> {
        self.value_types()
            .filter_map(DlrlMonoType::declared_name)
            .collect()
    }

    /// The type of every key and of every attribute's value or items.
    pub fn value_types(&self) -> impl Iterator<Item = &DlrlMonoType> {
        self.key_types
            .values()
            .chain(self.attributes.iter().map(|(_, ty)| ty.value_type()))
    }
}

/// Gather a flat pragma list into one entry per type, keyed by name.
///
/// A `DCPS_DATA_KEY` or `DCPS_DLRL_RELATION` for a type no
/// `DCPS_DATA_TYPE` declared still creates its entry. The alternative is
/// dropping it, and a key that silently does not reach the generated
/// class is the defect that shows up as an object with no identity at
/// run time rather than as an error here.
#[must_use]
pub fn collect_type_infos(pragmas: &[DlrlPragma]) -> BTreeMap<String, DlrlTypeInfo> {
    let mut out: BTreeMap<String, DlrlTypeInfo> = BTreeMap::new();
    let entry = |name: &String, out: &mut BTreeMap<String, DlrlTypeInfo>| {
        out.entry(name.clone()).or_insert_with(|| DlrlTypeInfo {
            name: name.clone(),
            ..DlrlTypeInfo::default()
        });
    };
    for pragma in pragmas {
        match pragma {
            DlrlPragma::DataType { name } => entry(name, &mut out),
            DlrlPragma::DataKey { type_name, field } => {
                entry(type_name, &mut out);
                if let Some(info) = out.get_mut(type_name) {
                    info.keys.push(field.clone());
                }
            }
            DlrlPragma::DlrlRelation {
                type_name,
                relation,
                target,
            } => {
                entry(type_name, &mut out);
                if let Some(info) = out.get_mut(type_name) {
                    info.relations.push((relation.clone(), target.clone()));
                }
            }
        }
    }
    out
}

/// The last segment of a scoped name.
#[must_use]
pub(crate) fn simple_name(scoped: &str) -> &str {
    scoped.rsplit("::").next().unwrap_or(scoped)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    fn pragmas() -> Vec<DlrlPragma> {
        vec![
            DlrlPragma::DataType {
                name: "demo::Trade".to_string(),
            },
            DlrlPragma::DataKey {
                type_name: "demo::Trade".to_string(),
                field: "id".to_string(),
            },
            DlrlPragma::DataKey {
                type_name: "demo::Trade".to_string(),
                field: "venue".to_string(),
            },
            DlrlPragma::DlrlRelation {
                type_name: "demo::Trade".to_string(),
                relation: "legs".to_string(),
                target: "demo::Leg".to_string(),
            },
        ]
    }

    #[test]
    fn everything_about_one_type_lands_in_one_entry() {
        let infos = collect_type_infos(&pragmas());
        assert_eq!(infos.len(), 1);
        let trade = infos.get("demo::Trade").expect("the type");
        assert_eq!(trade.keys, vec!["id".to_string(), "venue".to_string()]);
        assert_eq!(
            trade.relations,
            vec![("legs".to_string(), "demo::Leg".to_string())]
        );
    }

    /// Key order is the pragma order, because the IDL wrote it that way
    /// and a generated getter list that reorders them reads as a
    /// different type.
    #[test]
    fn key_order_is_the_order_the_pragmas_came_in() {
        let mut reversed = pragmas();
        reversed.swap(1, 2);
        let infos = collect_type_infos(&reversed);
        let trade = infos.get("demo::Trade").expect("the type");
        assert_eq!(trade.keys, vec!["venue".to_string(), "id".to_string()]);
    }

    /// A key for a type no `DCPS_DATA_TYPE` declared still produces an
    /// entry. Dropping it would make the key vanish from the generated
    /// class, which surfaces at run time as an object with no identity.
    #[test]
    fn a_key_without_its_data_type_still_creates_the_entry() {
        let infos = collect_type_infos(&[DlrlPragma::DataKey {
            type_name: "demo::Orphan".to_string(),
            field: "id".to_string(),
        }]);
        let orphan = infos.get("demo::Orphan").expect("the entry exists");
        assert_eq!(orphan.keys, vec!["id".to_string()]);
    }

    /// A resolved key is spelled in each language's mapping; an
    /// unresolved one as a type that does not exist there, never as a
    /// guessed integer.
    #[test]
    fn every_backend_spells_the_key_type_it_is_given() {
        let infos = collect_type_infos(&pragmas());
        let mut trade = infos.get("demo::Trade").expect("the type").clone();
        trade.key_types.insert(
            "venue".to_string(),
            DlrlMonoType::Basic(DlrlFieldType::String),
        );
        assert!(generate_cpp_object(&trade).contains("const char* get_venue()"));
        assert!(generate_java_object(&trade).contains("public String getVenue()"));
        assert!(generate_csharp_object(&trade).contains("public string Venue"));
        assert!(generate_ts_interface(&trade).contains("readonly venue: string;"));
        // `id` stayed unresolved.
        assert!(generate_cpp_object(&trade).contains("DLRL_UNRESOLVED_KEY_TYPE id_{};"));
        assert!(generate_java_object(&trade).contains("UnresolvedDlrlFieldType getId()"));
        assert!(generate_csharp_object(&trade).contains("UnresolvedDlrlFieldType Id"));
        assert!(generate_ts_class(&trade).contains("declare readonly id: never;"));
    }

    /// Every backend gives an attribute a getter and a setter in its
    /// language's type; a key keeps only its getter.
    #[test]
    fn every_backend_carries_every_attribute_with_a_getter_and_a_setter() {
        let infos = collect_type_infos(&pragmas());
        let mut trade = infos.get("demo::Trade").expect("the type").clone();
        let basic = |ty| DlrlAttributeType::Mono(DlrlMonoType::Basic(ty));
        trade.attributes = vec![
            ("price".to_string(), basic(DlrlFieldType::Double)),
            ("trader".to_string(), basic(DlrlFieldType::String)),
        ];
        let cpp = generate_cpp_object(&trade);
        assert!(cpp.contains("DDS::Double get_price() const"), "{cpp}");
        assert!(cpp.contains("void set_price(DDS::Double v)"), "{cpp}");
        assert!(cpp.contains("const char* get_trader() const"), "{cpp}");
        assert!(cpp.contains("void set_trader(const char* v)"), "{cpp}");
        assert!(!cpp.contains("set_id"), "{cpp}");
        let java = generate_java_object(&trade);
        assert!(java.contains("public double getPrice()"), "{java}");
        assert!(java.contains("public void setPrice(double v)"), "{java}");
        assert!(java.contains("public void setTrader(String v)"), "{java}");
        assert!(!java.contains("setId"), "{java}");
        let csharp = generate_csharp_object(&trade);
        assert!(
            csharp.contains("[DlrlMonoAttribute(\"price\")] public double Price { get; set; }"),
            "{csharp}"
        );
        assert!(
            csharp.contains("[DlrlMonoAttribute(\"trader\")] public string Trader { get; set; }"),
            "{csharp}"
        );
        let ts = generate_ts_interface(&trade);
        assert!(ts.contains("  price: number;"), "{ts}");
        assert!(ts.contains("  trader: string;"), "{ts}");
        let ts_class = generate_ts_class(&trade);
        assert!(ts_class.contains("  price: number = 0;"), "{ts_class}");
        assert!(ts_class.contains("  trader: string = \"\";"), "{ts_class}");
    }

    /// An enumeration and a structure are the types `idlc generate`
    /// declares for them; a sequence, an array and a map are a collection
    /// with a getter and no setter (§8.1.6.3.16–19: a collection is
    /// changed through its own `add`/`put`/`remove`).
    #[test]
    fn every_backend_carries_structured_and_multi_valued_attributes() {
        let infos = collect_type_infos(&pragmas());
        let mut trade = infos.get("demo::Trade").expect("the type").clone();
        let side = DlrlMonoType::Enum {
            name: "demo::Side".to_string(),
            default: "BUY".to_string(),
        };
        let money = DlrlMonoType::Struct {
            name: "demo::Money".to_string(),
        };
        let point = DlrlMonoType::Struct {
            name: "Point".to_string(),
        };
        trade.key_types.insert("venue".to_string(), side.clone());
        trade.attributes = vec![
            ("price".to_string(), DlrlAttributeType::Mono(money.clone())),
            ("direction".to_string(), DlrlAttributeType::Mono(side)),
            (
                "tags".to_string(),
                DlrlAttributeType::Multi {
                    basis: DlrlCollection::List,
                    item: DlrlMonoType::Basic(DlrlFieldType::String),
                },
            ),
            (
                "fees".to_string(),
                DlrlAttributeType::Multi {
                    basis: DlrlCollection::StrMap,
                    item: money,
                },
            ),
            (
                "path".to_string(),
                DlrlAttributeType::Multi {
                    basis: DlrlCollection::IntMap(DlrlFieldType::LongLong),
                    item: point,
                },
            ),
        ];
        assert_eq!(
            trade.declared_types().into_iter().collect::<Vec<_>>(),
            vec!["Point", "demo::Money", "demo::Side"]
        );

        let cpp = generate_cpp_object(&trade);
        for line in [
            "::demo::Side venue_{};",
            "const ::demo::Money& get_price() const { return price_; }",
            "void set_price(const ::demo::Money& v) { price_ = v; }",
            "void set_direction(::demo::Side v) { direction_ = v; }",
            "DLRL::List< DDS::String_var >& get_tags() { return tags_; }",
            "DLRL::StrMap< ::demo::Money > fees_{};",
            "DLRL::IntMap< DDS::LongLong, ::Point >& get_path()",
        ] {
            assert!(cpp.contains(line), "{line}\n{cpp}");
        }
        assert!(!cpp.contains("set_tags"), "{cpp}");

        let java = generate_java_object(&trade);
        for line in [
            "private int venue;",
            "public void setPrice(ZdTypes.demo.Money v)",
            "public void setDirection(int v)",
            "private final java.util.List<String> tags = new java.util.ArrayList<>();",
            "java.util.Map<String, ZdTypes.demo.Money> getFees()",
            "private final java.util.Map<Long, ZdTypes.Point> path = new java.util.LinkedHashMap<>();",
        ] {
            assert!(java.contains(line), "{line}\n{java}");
        }
        assert!(!java.contains("setTags"), "{java}");

        let csharp = generate_csharp_object(&trade);
        for line in [
            "[DlrlKey(\"venue\")] public global::demo.Side Venue { get; set; }",
            "[DlrlMonoAttribute(\"price\")] public global::demo.Money Price { get; set; }",
            "[DlrlMultiAttribute(\"tags\")] public System.Collections.Generic.List<string> Tags { get; } = new();",
            "System.Collections.Generic.Dictionary<long, global::Point> Path { get; } = new();",
        ] {
            assert!(csharp.contains(line), "{line}\n{csharp}");
        }

        let ts = generate_ts_interface(&trade);
        let ts_class = generate_ts_class(&trade);
        for (text, line) in [
            (&ts, "  readonly venue: demo.Side;"),
            (&ts, "  price: demo.Money;"),
            (&ts, "  readonly tags: string[];"),
            (&ts, "  readonly path: Map<bigint, Point>;"),
            (&ts_class, "  readonly venue: demo.Side = demo.Side.BUY;"),
            (&ts_class, "  price: demo.Money = demo.newMoney();"),
            (&ts_class, "  direction: demo.Side = demo.Side.BUY;"),
            (
                &ts_class,
                "  readonly fees: Map<string, demo.Money> = new Map();",
            ),
        ] {
            assert!(text.contains(line), "{line}\n{text}");
        }
        assert_eq!(
            generate_ts_import([&trade], "./trade").as_deref(),
            Some("import { Point, demo, newPoint } from \"./trade\";\n")
        );
        assert_eq!(
            generate_ts_import(infos.values(), "./trade"),
            None,
            "a class that names no IDL-declared type imports nothing"
        );
    }

    #[test]
    fn the_class_name_is_the_last_segment() {
        assert_eq!(simple_name("demo::nested::Trade"), "Trade");
        assert_eq!(simple_name("Trade"), "Trade");
    }

    /// Every backend names the same class and mentions every key and
    /// every relation. A backend that quietly drops one is what this
    /// catches — the four are written separately and only this holds
    /// them to the same content.
    #[test]
    fn every_backend_carries_every_key_and_relation() {
        let infos = collect_type_infos(&pragmas());
        let trade = infos.get("demo::Trade").expect("the type");
        for generated in [
            generate_cpp_object(trade),
            generate_csharp_object(trade),
            generate_java_object(trade),
            generate_ts_interface(trade),
        ] {
            assert!(generated.contains("Trade"), "{generated}");
            assert!(generated.contains("id"), "{generated}");
            assert!(generated.contains("venue"), "{generated}");
            assert!(generated.contains("legs"), "{generated}");
            assert!(generated.contains("Leg"), "{generated}");
        }
    }

    #[test]
    fn every_home_names_its_object_class() {
        let infos = collect_type_infos(&pragmas());
        let trade = infos.get("demo::Trade").expect("the type");
        for generated in [
            generate_cpp_home(trade),
            generate_csharp_home(trade),
            generate_java_home(trade),
            generate_ts_class(trade),
        ] {
            assert!(generated.contains("Trade"), "{generated}");
        }
    }
}
