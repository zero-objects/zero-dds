// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-corba-rt`. Safety classification: **STANDARD**.
//!
//! OMG **Real-Time CORBA** 1.0 (`formal/2005-01-04`) — pure-Rust `no_std + alloc`,
//! `forbid(unsafe_code)`. Brings deterministic real-time behavior to the CORBA
//! model: end-to-end priority propagation, priority-aware thread pools
//! and priority-banded connections.
//!
//! Implements:
//! - **`Priority`** + **`PriorityMapping`** (§5.3/§5.4) — CORBA priority
//!   (`0..=32767`, negative values reserved) ↔ native OS priority ([`priority`]).
//! - **`PriorityModel`** (§5.4.1) — `SERVER_DECLARED` vs `CLIENT_PROPAGATED` +
//!   `PriorityModelPolicy`; **Threadpool**/**Lane** (§5.7) + **PriorityBand**
//!   (§5.8) with priority-based selection ([`policy`]).
//! - **`RTCorbaPriority` ServiceContext** (id 10, §5.4.2) — propagation of the
//!   client priority over GIOP ([`propagation`]).
//! - **`RtCurrent`** (§5.5) — read/write access to the CORBA priority of the
//!   current context ([`current`]).
//! - **`RtMutex`** + **`PriorityInheritance`** (§5.6) — a blocking mutex
//!   with a priority-inheritance protocol against priority inversion ([`mutex`]).
//! - **`ThreadpoolRuntime`** (§5.7) — the runnable lanes-on-OS-threads
//!   realization with an injectable native priority hook ([`threadpool`]).
//! - **`TaskSet`** (scheduling profile) — static schedulability analysis
//!   (Liu & Layland bound + exact response-time analysis) ([`scheduling`]).
//!
//! It also carries the CORBA types the whole CORBA column needs and
//! that no wire runtime should: **`TypeCode`** (CORBA 3.3 part 2 §15.3.5,
//! [`type_code`]), **`any`** with its `wstring` (§15.3.7 and §15.3.2.7,
//! [`any`]), and **`ObjectReference`** with **`ValueBase`** (§13.6.2 and
//! OMG IDL 4.2 rule (131), [`object`]). `zerodds-wire` reads and writes
//! bytes at the alignment a layout plan names; a self-describing type
//! graph is a model, and this is the CORBA runtime the services above
//! already sit on.
//!
//! Those last two arrived because `zerodds-corba-codegen`'s Annex A.1
//! table already named this crate for them and nothing here declared
//! them. A table row is a string until something compiles what it says.
//!
//! Spec: OMG Real-Time CORBA 1.0. Complementary to the DDS real-time QoS side
//! (Deadline/Latency-Budget/Transport-Priority) for legacy CORBA code.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod any;
pub mod current;
pub mod mutex;
pub mod object;
pub mod policy;
pub mod priority;
pub mod propagation;
pub mod scheduling;
pub mod threadpool;
pub mod type_code;

pub use any::{AnyValue, CorbaAny, WString};
pub use current::RtCurrent;
pub use mutex::PriorityInheritance;
pub use object::{ObjectReference, ValueBase};
pub use policy::{
    Lane, PriorityBand, PriorityBandedConnectionPolicy, PriorityModel, PriorityModelPolicy,
    Threadpool, ThreadpoolPolicy,
};
pub use priority::{LinearPriorityMapping, Priority, PriorityMapping};
pub use propagation::{RT_CORBA_PRIORITY_SC_ID, decode_priority_context, encode_priority_context};
pub use scheduling::{Task, TaskSet};
pub use type_code::{MAX_TYPECODE_DEPTH, TypeCode};

#[cfg(feature = "std")]
pub use mutex::{RtMutex, RtMutexGuard};
#[cfg(feature = "std")]
pub use threadpool::{DispatchError, NativePrioritySetter, ThreadpoolRuntime};
