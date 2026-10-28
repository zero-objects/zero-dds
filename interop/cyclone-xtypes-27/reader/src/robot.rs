// SPDX-License-Identifier: Apache-2.0
//! The matrix's `Robot`, generated at build time from `../robot.idl`
//! under the extensibility `build.rs` was told (see
//! `ZERODDS_ROBOT_EXTENSIBILITY` there).
//!
//! The `DdsType` impl is generated with it. It used to stand here by
//! hand, and a hand-written bridge answers the wire questions a second
//! time: this one left `DdsType::type_object()` at its default `None`,
//! so the endpoint registered no `TypeObject` with the participant's
//! TypeLookup server and its SEDP announcement carried no
//! `PID_TYPE_INFORMATION` (XTypes 1.3 §7.6.3.2) — the endpoint told a
//! peer its topic type name and nothing about the type. The matrix
//! exists to measure exactly that, so the bridge it measures through is
//! the generated one.

include!(concat!(env!("OUT_DIR"), "/robot.rs"));
