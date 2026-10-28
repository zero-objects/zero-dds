// SPDX-License-Identifier: Apache-2.0
//! Shared types for the issue #32 harness.
//!
//! `Robot` is generated at build time from `idl/Robot.idl` by
//! `zerodds-build`; `src/robot.rs` includes the generated module and
//! carries the `DdsType` bridge with the octets pinned in its tests.
//! The previous form — a checked-in fixture of the retired
//! `zerodds-idlc` generation over `zerodds-cdr` — stopped building when
//! the rebuild removed both.
pub mod robot;
