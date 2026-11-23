// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! GIOP dispatch for `CosTime::TimeService`, `CosTime::UTO` and
//! `CosTime::TIO` — OMG Time Service 1.1 §1.2, §1.3.4, §1.3.5, §2.1.
//! Feature `serve`.
//!
//! The service object (§1.2) is one CORBA object with the object key
//! [`TIME_SERVICE_OBJECT_KEY`] (`corbaloc::host:port/TimeService`). The
//! UTOs and TIOs it creates are CORBA objects of their own. Since a UTO
//! and a TIO are immutable (§1.3.4: "It is intended that UTOs are
//! immutable"), the object key carries the value: `CosTime/UTO/` and the
//! 16 octets of the `UtcT`, `CosTime/TIO/` and the 16 octets of the
//! `IntervalT`, both big-endian. A reference stays valid for as long as
//! a server with this servant listens at its address, and the servant
//! keeps no table of objects that would grow with every call.
//!
//! A UTO or TIO passed in as a parameter (`compare_time`,
//! `time_to_interval`, `spans`, `overlaps`) is read from its object key
//! when it is one of these keys; a reference to another server's object
//! is read with `_get_utc_time` / `_get_time_interval` over IIOP. A nil
//! reference, or one that cannot be read, raises `CORBA::BAD_PARAM`
//! (§1.3.5.2: "The exception CORBA::BAD_PARAM is raised if the UTO passed
//! in is invalid").
//!
//! Current time over the network (App. B.2, "Proxies and Time
//! Uncertainty"): a time that crosses the network reaches the caller
//! later than the service generated it, by a latency the service does
//! not know. Two operations read the clock of the service:
//! `universal_time` (§2.1.1) and `absolute_time` of a UTO (§1.3.4.5,
//! "current time + time in the object"). Both follow the
//! [`RemoteTimePolicy`]. By default ([`RemoteTimePolicy::Refuse`]) both
//! raise `CORBA::NO_PERMISSION`: no current time of the service reaches
//! a caller in another address space, which uses a Time Service in its
//! own (the first way of B.2). With
//! [`RemoteTimePolicy::AnswerCorrectingProxies`] both answer, for
//! callers that correct the latency themselves such as
//! [`crate::proxy::TimeServiceProxy`] (the second way of B.2).
//! `secure_universal_time` raises `TimeUnavailable` under either policy
//! (§2.1.2: secure time cannot be guaranteed across IIOP).
//!
//! Under `Refuse` the served objects deviate from §2.1.1 ("If any time is
//! available at all, that time is returned by this operation") and from
//! §1.3.4.5, as B.2 requires ("Implementors of the Time Service must
//! prevent this problem from occurring"). The refusal is the system
//! exception `NO_PERMISSION` ("the caller has insufficient privileges",
//! CORBA 3.3 Part 1 §8.12.3.7), not `TimeUnavailable`: that user
//! exception keeps the meaning §2.1.1 gives it, the failure of the
//! underlying time provider, and `absolute_time` has no user exception.
//!
//! Exceptions: `CosTime::TimeUnavailable` is a user exception
//! (§1.3.3.1); `BAD_PARAM` (§2.1.2.1, §2.1.2.3) and `DATA_CONVERSION`
//! (§1.3.4.5, §1.3.5.4) are system exceptions. `absolute_time` on a
//! service whose clock gives no time raises `CORBA::TRANSIENT`, since the
//! IDL of `absolute_time` names no user exception.

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use zerodds_corba_giop::object_ops::{self, ObjectOpReply};
use zerodds_corba_giop::{
    Message, ReplyStatusType, Request, ResponseFlags, TargetAddress, Version,
};
use zerodds_corba_iiop::{Connector, ConnectorConfig, IiopProfileBody, IiopVersion};
use zerodds_corba_ior::{Ior, TaggedProfile};
use zerodds_wire::{Endianness, Reader, Writer};

use crate::exception::AbsoluteTimeError;
use crate::service::TimeService;
use crate::time_base::{IntervalT, UtcT};
use crate::tio::{OverlapType, Tio};
use crate::uto::{ComparisonType, TimeComparison, Uto};

/// Repository id of `CosTime::TimeService`.
pub const TIME_SERVICE_TYPE_ID: &str = "IDL:omg.org/CosTime/TimeService:1.0";
/// Repository id of `CosTime::UTO`.
pub const UTO_TYPE_ID: &str = "IDL:omg.org/CosTime/UTO:1.0";
/// Repository id of `CosTime::TIO`.
pub const TIO_TYPE_ID: &str = "IDL:omg.org/CosTime/TIO:1.0";
/// Repository id of the user exception `CosTime::TimeUnavailable`.
pub const TIME_UNAVAILABLE_TYPE_ID: &str = "IDL:omg.org/CosTime/TimeUnavailable:1.0";
/// Object key of the service object.
pub const TIME_SERVICE_OBJECT_KEY: &[u8] = b"TimeService";

const UTO_KEY_PREFIX: &[u8] = b"CosTime/UTO/";
const TIO_KEY_PREFIX: &[u8] = b"CosTime/TIO/";

/// How long reading a foreign UTO or TIO may take.
const REMOTE_TIMEOUT: Duration = Duration::from_secs(5);

/// How the servant answers the two operations that read the clock of the
/// service, `universal_time` and `absolute_time` of a UTO (OMG Time
/// Service 1.1 App. B.2). `secure_universal_time` raises
/// `TimeUnavailable` under either policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RemoteTimePolicy {
    /// `universal_time` and `absolute_time` raise `CORBA::NO_PERMISSION`:
    /// no current time of the service leaves its address space, so no
    /// proxy delivers a stale one (B.2: "Prohibit proxies of the time
    /// server object"). The operations that read no clock are served.
    #[default]
    Refuse,
    /// `universal_time` and `absolute_time` answer with the time of the
    /// service at the moment it generates the UTO. For callers that
    /// measure the latency and widen the interval before they use it, as
    /// [`crate::proxy::TimeServiceProxy`] does (B.2: "Create a special
    /// time server proxy"); the servant cannot check that a caller does.
    AnswerCorrectingProxies,
}

/// The system exception of an operation that reads the clock of the
/// service under [`RemoteTimePolicy::Refuse`].
pub const REFUSED_REMOTE_TIME: &str = "NO_PERMISSION";

/// The result of one dispatched request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServantReply {
    /// `NO_EXCEPTION`: the encoded return value and out parameters.
    Ok(Vec<u8>),
    /// `USER_EXCEPTION`: the complete body (`string` repository id and
    /// the members; `TimeUnavailable` has none).
    UserException {
        /// Repository id, also at the start of `body`.
        repository_id: &'static str,
        /// The reply body.
        body: Vec<u8>,
    },
    /// `SYSTEM_EXCEPTION` with the name of the standard exception
    /// (`BAD_PARAM`, `DATA_CONVERSION`, `TRANSIENT`, `NO_PERMISSION`,
    /// `MARSHAL`, `BAD_OPERATION`, `OBJECT_NOT_EXIST`, `INTF_REPOS`); the
    /// body is [`system_exception_body`].
    SystemException(&'static str),
}

/// The reply body of a standard system exception (CORBA 3.3 Part 2
/// §15.4.3.2): repository id, minor code 0, `COMPLETED_NO`.
#[must_use]
pub fn system_exception_body(name: &str, endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    w.write_string(&alloc::format!("IDL:omg.org/CORBA/{name}:1.0"));
    w.write_u32(0);
    w.write_u32(1);
    w.finish()
}

/// What an object key names.
enum Target {
    Service,
    Uto(Uto),
    Tio(Tio),
}

/// The object key of a UTO with the value `utc`.
#[must_use]
pub fn uto_object_key(utc: UtcT) -> Vec<u8> {
    let mut key = UTO_KEY_PREFIX.to_vec();
    key.extend_from_slice(&utc.time.to_be_bytes());
    key.extend_from_slice(&utc.inacclo.to_be_bytes());
    key.extend_from_slice(&utc.inacchi.to_be_bytes());
    key.extend_from_slice(&utc.tdf.to_be_bytes());
    key
}

/// The object key of a TIO with the value `interval`.
#[must_use]
pub fn tio_object_key(interval: IntervalT) -> Vec<u8> {
    let mut key = TIO_KEY_PREFIX.to_vec();
    key.extend_from_slice(&interval.lower_bound.to_be_bytes());
    key.extend_from_slice(&interval.upper_bound.to_be_bytes());
    key
}

fn be_u64(b: &[u8]) -> Option<u64> {
    Some(u64::from_be_bytes(b.try_into().ok()?))
}

fn parse_key(key: &[u8]) -> Option<Target> {
    if key == TIME_SERVICE_OBJECT_KEY {
        return Some(Target::Service);
    }
    if let Some(v) = key.strip_prefix(UTO_KEY_PREFIX) {
        if v.len() != 16 {
            return None;
        }
        let utc = UtcT {
            time: be_u64(&v[0..8])?,
            inacclo: u32::from_be_bytes(v[8..12].try_into().ok()?),
            inacchi: u16::from_be_bytes(v[12..14].try_into().ok()?),
            tdf: i16::from_be_bytes(v[14..16].try_into().ok()?),
        };
        return Some(Target::Uto(Uto::from_utc(utc)));
    }
    if let Some(v) = key.strip_prefix(TIO_KEY_PREFIX) {
        if v.len() != 16 {
            return None;
        }
        let interval = IntervalT::new(be_u64(&v[0..8])?, be_u64(&v[8..16])?)?;
        return Some(Target::Tio(Tio::from_interval(interval)));
    }
    None
}

fn write_utc(w: &mut Writer, utc: UtcT) {
    w.write_u64(utc.time);
    w.write_u32(utc.inacclo);
    w.write_u16(utc.inacchi);
    w.write_i16(utc.tdf);
}

fn read_utc(r: &mut Reader<'_>) -> Option<UtcT> {
    Some(UtcT {
        time: r.read_u64().ok()?,
        inacclo: r.read_u32().ok()?,
        inacchi: r.read_u16().ok()?,
        tdf: r.read_i16().ok()?,
    })
}

fn write_interval(w: &mut Writer, i: IntervalT) {
    w.write_u64(i.lower_bound);
    w.write_u64(i.upper_bound);
}

fn read_interval(r: &mut Reader<'_>) -> Option<(u64, u64)> {
    Some((r.read_u64().ok()?, r.read_u64().ok()?))
}

const fn comparison_code(c: TimeComparison) -> u32 {
    match c {
        TimeComparison::EqualTo => 0,
        TimeComparison::LessThan => 1,
        TimeComparison::GreaterThan => 2,
        TimeComparison::Indeterminate => 3,
    }
}

const fn overlap_code(o: OverlapType) -> u32 {
    match o {
        OverlapType::Container => 0,
        OverlapType::Contained => 1,
        OverlapType::Overlap => 2,
        OverlapType::NoOverlap => 3,
    }
}

/// The object key of the first IIOP profile of `ior`, with its address.
fn iiop_of(ior: &Ior) -> Option<IiopProfileBody> {
    ior.profiles
        .iter()
        .find_map(TaggedProfile::as_iiop)
        .and_then(Result::ok)
}

/// GIOP dispatcher for one [`TimeService`].
pub struct TimeServant {
    service: TimeService,
    remote_time: RemoteTimePolicy,
    /// The address every IOR this servant returns advertises.
    host: String,
    port: u16,
    connector: Connector,
    next_request_id: AtomicU32,
}

impl core::fmt::Debug for TimeServant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TimeServant")
            .field("host", &self.host)
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

impl TimeServant {
    /// A servant over `service`, advertising `host:port` in the IORs of
    /// the UTOs and TIOs it returns.
    #[must_use]
    pub fn new(service: TimeService, host: String, port: u16) -> Self {
        let config = ConnectorConfig {
            connect_timeout: Some(REMOTE_TIMEOUT),
            read_timeout: Some(REMOTE_TIMEOUT),
            write_timeout: Some(REMOTE_TIMEOUT),
            ..ConnectorConfig::default()
        };
        Self {
            service,
            remote_time: RemoteTimePolicy::default(),
            host,
            port,
            connector: Connector::new(config),
            next_request_id: AtomicU32::new(1),
        }
    }

    /// The servant with `policy` for `universal_time` and `absolute_time`
    /// (App. B.2).
    #[must_use]
    pub fn with_remote_time(mut self, policy: RemoteTimePolicy) -> Self {
        self.remote_time = policy;
        self
    }

    /// The policy for `universal_time` and `absolute_time`.
    #[must_use]
    pub fn remote_time(&self) -> RemoteTimePolicy {
        self.remote_time
    }

    /// `Ok` if the policy lets the current time of the service go to the
    /// caller, else [`REFUSED_REMOTE_TIME`].
    fn current_time_allowed(&self) -> Result<(), Raise> {
        match self.remote_time {
            RemoteTimePolicy::Refuse => Err(Raise::System(REFUSED_REMOTE_TIME)),
            RemoteTimePolicy::AnswerCorrectingProxies => Ok(()),
        }
    }

    /// The IOR of the service object.
    ///
    /// # Errors
    /// The text of the IOR encoder's error.
    pub fn service_ior(&self) -> Result<Ior, String> {
        self.ior(TIME_SERVICE_OBJECT_KEY.to_vec(), TIME_SERVICE_TYPE_ID)
    }

    /// The IOR of a UTO with the value `uto`.
    ///
    /// # Errors
    /// The text of the IOR encoder's error.
    pub fn uto_ior(&self, uto: Uto) -> Result<Ior, String> {
        self.ior(uto_object_key(uto.utc_time()), UTO_TYPE_ID)
    }

    /// The IOR of a TIO with the value `tio`.
    ///
    /// # Errors
    /// The text of the IOR encoder's error.
    pub fn tio_ior(&self, tio: Tio) -> Result<Ior, String> {
        self.ior(tio_object_key(tio.time_interval()), TIO_TYPE_ID)
    }

    fn ior(&self, key: Vec<u8>, type_id: &str) -> Result<Ior, String> {
        let body = IiopProfileBody::new(IiopVersion::V1_2, self.host.clone(), self.port, key);
        let profile =
            TaggedProfile::iiop(&body, Endianness::Little).map_err(|e| alloc::format!("{e:?}"))?;
        Ok(Ior {
            type_id: String::from(type_id),
            profiles: alloc::vec![profile],
        })
    }

    /// `true` if `object_key` names an object of this servant (for a
    /// GIOP `LocateRequest`).
    #[must_use]
    pub fn knows(&self, object_key: &[u8]) -> bool {
        parse_key(object_key).is_some()
    }

    /// Dispatches one request; `object_key`, `operation`, `body` and
    /// `endianness` are the fields of the GIOP request.
    #[must_use]
    pub fn dispatch(
        &self,
        object_key: &[u8],
        operation: &str,
        body: &[u8],
        endianness: Endianness,
    ) -> ServantReply {
        self.dispatch_at(object_key, operation, body, 0, 0, endianness)
    }

    /// As [`Self::dispatch`], for a body that does not start on an
    /// 8-octet boundary of its message. CDR aligns every value from the
    /// start of the GIOP message (CORBA 3.3 Part 2 §15.4.1). GIOP 1.2 puts
    /// the body of a request and of a reply on an 8-octet boundary
    /// (§15.4.2.2, §15.4.3.1), GIOP 1.0 and 1.1 do not: there the body
    /// follows the header directly, and a `unsigned long long` argument is
    /// padded relative to the message. `request_body_offset` and
    /// `reply_body_offset` are the offsets of the two bodies in their
    /// messages; only their remainder modulo 8 matters.
    #[must_use]
    pub fn dispatch_at(
        &self,
        object_key: &[u8],
        operation: &str,
        body: &[u8],
        request_body_offset: usize,
        reply_body_offset: usize,
        endianness: Endianness,
    ) -> ServantReply {
        let Some(target) = parse_key(object_key) else {
            return match object_ops::unknown_object_reply(operation, endianness) {
                Some(b) => ServantReply::Ok(b),
                None => ServantReply::SystemException("OBJECT_NOT_EXIST"),
            };
        };
        let type_id = match target {
            Target::Service => TIME_SERVICE_TYPE_ID,
            Target::Uto(_) => UTO_TYPE_ID,
            Target::Tio(_) => TIO_TYPE_ID,
        };
        if let Some(reply) = object_ops::dispatch_with_bases(
            operation,
            &zerodds_corba_giop::AlignedBody::new(body, request_body_offset),
            endianness,
            type_id,
            &[],
        ) {
            return match reply {
                ObjectOpReply::Ok(b) => ServantReply::Ok(b),
                ObjectOpReply::Marshal => ServantReply::SystemException("MARSHAL"),
                ObjectOpReply::InterfaceRepositoryUnavailable => {
                    ServantReply::SystemException("INTF_REPOS")
                }
            };
        }
        // The body behind as many octets as it is off an 8-octet
        // boundary, so the reader and the writer pad as the message does.
        let in_pad = request_body_offset % 8;
        let mut placed = alloc::vec![0u8; in_pad];
        placed.extend_from_slice(body);
        let mut r = Reader::new(&placed, endianness);
        if r.read_bytes(in_pad).is_err() {
            return ServantReply::SystemException("MARSHAL");
        }
        let out_pad = reply_body_offset % 8;
        let mut w = Writer::new(endianness);
        for _ in 0..out_pad {
            w.write_u8(0);
        }
        let outcome = match target {
            Target::Service => self.service_op(operation, &mut r, &mut w),
            Target::Uto(uto) => self.uto_op(uto, operation, &mut r, &mut w),
            Target::Tio(tio) => self.tio_op(tio, operation, &mut r, &mut w),
        };
        match outcome {
            Ok(()) => {
                let mut out = w.finish();
                out.drain(..out_pad);
                ServantReply::Ok(out)
            }
            Err(Raise::TimeUnavailable) => {
                let mut e = Writer::new(endianness);
                e.write_string(TIME_UNAVAILABLE_TYPE_ID);
                ServantReply::UserException {
                    repository_id: TIME_UNAVAILABLE_TYPE_ID,
                    body: e.finish(),
                }
            }
            Err(Raise::System(name)) => ServantReply::SystemException(name),
        }
    }

    fn write_object(&self, w: &mut Writer, ior: Result<Ior, String>) -> Result<(), Raise> {
        let ior = ior.map_err(|_| Raise::System("MARSHAL"))?;
        ior.encode(w).map_err(|_| Raise::System("MARSHAL"))
    }

    fn service_op(&self, op: &str, r: &mut Reader<'_>, w: &mut Writer) -> Result<(), Raise> {
        match op {
            "universal_time" => {
                self.current_time_allowed()?;
                let uto = self
                    .service
                    .universal_time()
                    .map_err(|_| Raise::TimeUnavailable)?;
                self.write_object(w, self.uto_ior(uto))
            }
            // Secure time stays in the address space of the service: IIOP
            // does not protect it in transit (App. B.1.1) and the latency
            // to the caller is not known (App. B.2).
            "secure_universal_time" => Err(Raise::TimeUnavailable),
            "new_universal_time" => {
                let time = r.read_u64().map_err(|_| Raise::System("MARSHAL"))?;
                let inaccuracy = r.read_u64().map_err(|_| Raise::System("MARSHAL"))?;
                let tdf = r.read_i16().map_err(|_| Raise::System("MARSHAL"))?;
                let uto = TimeService::new_universal_time(time, inaccuracy, tdf)
                    .map_err(|_| Raise::System("BAD_PARAM"))?;
                self.write_object(w, self.uto_ior(uto))
            }
            "uto_from_utc" => {
                let utc = read_utc(r).ok_or(Raise::System("MARSHAL"))?;
                self.write_object(w, self.uto_ior(TimeService::uto_from_utc(utc)))
            }
            "new_interval" => {
                let (lower, upper) = read_interval(r).ok_or(Raise::System("MARSHAL"))?;
                let tio = TimeService::new_interval(lower, upper)
                    .map_err(|_| Raise::System("BAD_PARAM"))?;
                self.write_object(w, self.tio_ior(tio))
            }
            _ => Err(Raise::System("BAD_OPERATION")),
        }
    }

    fn uto_op(&self, uto: Uto, op: &str, r: &mut Reader<'_>, w: &mut Writer) -> Result<(), Raise> {
        match op {
            "_get_time" => w.write_u64(uto.time()),
            "_get_inaccuracy" => w.write_u64(uto.inaccuracy()),
            "_get_tdf" => w.write_i16(uto.tdf()),
            "_get_utc_time" => write_utc(w, uto.utc_time()),
            "absolute_time" => {
                // Current time + time in the object (§1.3.4.5): the
                // current time of the service, as in `universal_time`.
                self.current_time_allowed()?;
                let absolute = self.service.absolute_time(uto).map_err(|e| match e {
                    AbsoluteTimeError::DataConversion => Raise::System("DATA_CONVERSION"),
                    AbsoluteTimeError::TimeUnavailable => Raise::System("TRANSIENT"),
                })?;
                return self.write_object(w, self.uto_ior(absolute));
            }
            "compare_time" => {
                let kind = match r.read_u32().map_err(|_| Raise::System("MARSHAL"))? {
                    0 => ComparisonType::IntervalC,
                    1 => ComparisonType::MidC,
                    _ => return Err(Raise::System("BAD_PARAM")),
                };
                let other = self.read_uto(r)?;
                w.write_u32(comparison_code(uto.compare_time(kind, other)));
            }
            "time_to_interval" => {
                let other = self.read_uto(r)?;
                let tio = uto
                    .time_to_interval(other)
                    .ok_or(Raise::System("BAD_PARAM"))?;
                return self.write_object(w, self.tio_ior(tio));
            }
            "interval" => return self.write_object(w, self.tio_ior(uto.interval())),
            _ => return Err(Raise::System("BAD_OPERATION")),
        }
        Ok(())
    }

    fn tio_op(&self, tio: Tio, op: &str, r: &mut Reader<'_>, w: &mut Writer) -> Result<(), Raise> {
        match op {
            "_get_time_interval" => write_interval(w, tio.time_interval()),
            "spans" => {
                let other = self.read_uto(r)?;
                let (kind, overlap) = tio.spans(other);
                w.write_u32(overlap_code(kind));
                return self.write_object(w, self.tio_ior(overlap));
            }
            "overlaps" => {
                let other = self.read_tio(r)?;
                let (kind, overlap) = tio.overlaps(other);
                w.write_u32(overlap_code(kind));
                return self.write_object(w, self.tio_ior(overlap));
            }
            "time" => {
                let uto = tio.time().map_err(|_| Raise::System("DATA_CONVERSION"))?;
                return self.write_object(w, self.uto_ior(uto));
            }
            _ => return Err(Raise::System("BAD_OPERATION")),
        }
        Ok(())
    }

    /// An `in UTO` parameter: from its object key, else read from its
    /// server; `BAD_PARAM` if neither works.
    fn read_uto(&self, r: &mut Reader<'_>) -> Result<Uto, Raise> {
        let ior = Ior::decode(r).map_err(|_| Raise::System("MARSHAL"))?;
        let profile = iiop_of(&ior).ok_or(Raise::System("BAD_PARAM"))?;
        if let Some(Target::Uto(uto)) = parse_key(&profile.object_key) {
            return Ok(uto);
        }
        let body = self
            .remote_get(&profile, "_get_utc_time")
            .ok_or(Raise::System("BAD_PARAM"))?;
        let (bytes, endianness) = body;
        let mut rr = Reader::new(&bytes, endianness);
        read_utc(&mut rr)
            .map(Uto::from_utc)
            .ok_or(Raise::System("BAD_PARAM"))
    }

    /// An `in TIO` parameter, as [`Self::read_uto`].
    fn read_tio(&self, r: &mut Reader<'_>) -> Result<Tio, Raise> {
        let ior = Ior::decode(r).map_err(|_| Raise::System("MARSHAL"))?;
        let profile = iiop_of(&ior).ok_or(Raise::System("BAD_PARAM"))?;
        if let Some(Target::Tio(tio)) = parse_key(&profile.object_key) {
            return Ok(tio);
        }
        let (bytes, endianness) = self
            .remote_get(&profile, "_get_time_interval")
            .ok_or(Raise::System("BAD_PARAM"))?;
        let mut rr = Reader::new(&bytes, endianness);
        let (lower, upper) = read_interval(&mut rr).ok_or(Raise::System("BAD_PARAM"))?;
        IntervalT::new(lower, upper)
            .map(Tio::from_interval)
            .ok_or(Raise::System("BAD_PARAM"))
    }

    /// Reads an attribute of an object on another server: one GIOP 1.2
    /// request, the reply body if it is `NO_EXCEPTION`.
    fn remote_get(
        &self,
        profile: &IiopProfileBody,
        operation: &str,
    ) -> Option<(Vec<u8>, Endianness)> {
        let mut pooled = self.connector.connect(&profile.host, profile.port).ok()?;
        let conn = pooled.connection()?;
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let request = Request::new(
            request_id,
            ResponseFlags::SYNC_WITH_TARGET,
            TargetAddress::Key(profile.object_key.clone()),
            String::from(operation),
        );
        conn.write_message(
            Version::V1_2,
            Endianness::Little,
            false,
            &Message::Request(request),
        )
        .ok()?;
        loop {
            let (msg, endianness, _) = conn.read_message_full().ok()?;
            if let Message::Reply(reply) = msg {
                if reply.request_id != request_id {
                    continue;
                }
                return (reply.reply_status == ReplyStatusType::NoException)
                    .then_some((reply.body, endianness));
            }
        }
    }
}

/// What an operation raises.
enum Raise {
    TimeUnavailable,
    System(&'static str),
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    const E: Endianness = Endianness::Little;

    fn servant() -> TimeServant {
        TimeServant::new(
            TimeService::with_source(|| 1_000_000).with_tdf(60),
            String::from("127.0.0.1"),
            2_809,
        )
        .with_remote_time(RemoteTimePolicy::AnswerCorrectingProxies)
    }

    fn ok(reply: ServantReply) -> Vec<u8> {
        match reply {
            ServantReply::Ok(b) => b,
            other => panic!("expected NO_EXCEPTION, got {other:?}"),
        }
    }

    fn key_of(body: &[u8], endianness: Endianness) -> (String, Vec<u8>) {
        let mut r = Reader::new(body, endianness);
        let ior = Ior::decode(&mut r).unwrap();
        (ior.type_id.clone(), iiop_of(&ior).unwrap().object_key)
    }

    fn ior_body(ior: &Ior, endianness: Endianness) -> Vec<u8> {
        let mut w = Writer::new(endianness);
        ior.encode(&mut w).unwrap();
        w.finish()
    }

    #[test]
    fn time_1_2_universal_time_returns_a_uto_reference_with_the_value_in_its_key() {
        let s = servant();
        for e in [Endianness::Little, Endianness::Big] {
            let body = ok(s.dispatch(TIME_SERVICE_OBJECT_KEY, "universal_time", &[], e));
            let (type_id, key) = key_of(&body, e);
            assert_eq!(type_id, UTO_TYPE_ID);
            let time = ok(s.dispatch(&key, "_get_time", &[], e));
            assert_eq!(Reader::new(&time, e).read_u64().unwrap(), 1_000_000);
            let tdf = ok(s.dispatch(&key, "_get_tdf", &[], e));
            assert_eq!(Reader::new(&tdf, e).read_i16().unwrap(), 60);
        }
    }

    #[test]
    fn time_1_2_time_unavailable_is_the_cos_time_user_exception() {
        let s = TimeServant::new(TimeService::with_source(|| 0), String::from("h"), 1)
            .with_remote_time(RemoteTimePolicy::AnswerCorrectingProxies);
        match s.dispatch(TIME_SERVICE_OBJECT_KEY, "universal_time", &[], E) {
            ServantReply::UserException {
                repository_id,
                body,
            } => {
                assert_eq!(repository_id, TIME_UNAVAILABLE_TYPE_ID);
                assert_eq!(
                    Reader::new(&body, E).read_string().unwrap(),
                    TIME_UNAVAILABLE_TYPE_ID
                );
            }
            other => panic!("{other:?}"),
        }
        // secure_universal_time without a secure source: TimeUnavailable.
        assert!(matches!(
            servant().dispatch(TIME_SERVICE_OBJECT_KEY, "secure_universal_time", &[], E),
            ServantReply::UserException { .. }
        ));
    }

    fn is_time_unavailable(reply: ServantReply) -> bool {
        matches!(
            reply,
            ServantReply::UserException {
                repository_id: TIME_UNAVAILABLE_TYPE_ID,
                ..
            }
        )
    }

    /// `new_universal_time(time, inaccuracy, 0)` on `s`: the object key of
    /// the UTO it returns.
    fn new_uto(s: &TimeServant, time: u64, inaccuracy: u64) -> Vec<u8> {
        let mut w = Writer::new(E);
        w.write_u64(time);
        w.write_u64(inaccuracy);
        w.write_i16(0);
        let body = ok(s.dispatch(
            TIME_SERVICE_OBJECT_KEY,
            "new_universal_time",
            &w.finish(),
            E,
        ));
        key_of(&body, E).1
    }

    #[test]
    fn time_b_2_universal_time_is_refused_by_default() {
        let s = TimeServant::new(TimeService::with_source(|| 1_000_000), String::from("h"), 1);
        assert_eq!(s.remote_time(), RemoteTimePolicy::Refuse);
        // The clock gives a time, so the refusal is not TimeUnavailable
        // (§2.1.1) but the system exception NO_PERMISSION.
        assert_eq!(
            s.dispatch(TIME_SERVICE_OBJECT_KEY, "universal_time", &[], E),
            ServantReply::SystemException("NO_PERMISSION")
        );
        // Operations without current time are served.
        let key = new_uto(&s, 5, 1);
        let utc = ok(s.dispatch(&key, "_get_utc_time", &[], E));
        let utc = read_utc(&mut Reader::new(&utc, E)).unwrap();
        assert_eq!((utc.time, utc.inaccuracy()), (5, 1));
    }

    #[test]
    fn time_b_2_absolute_time_is_refused_by_default() {
        // What a remote caller would do to read the current time without
        // `universal_time`: a UTO of time 0, then its absolute time
        // (§1.3.4.5: current time + time in the object).
        for policy in [
            RemoteTimePolicy::Refuse,
            RemoteTimePolicy::AnswerCorrectingProxies,
        ] {
            let s = TimeServant::new(
                TimeService::with_source(|| 1_000_000).with_tdf(0),
                String::from("h"),
                1,
            )
            .with_remote_time(policy);
            let zero = new_uto(&s, 0, 0);
            let reply = s.dispatch(&zero, "absolute_time", &[], E);
            if policy == RemoteTimePolicy::Refuse {
                assert_eq!(reply, ServantReply::SystemException("NO_PERMISSION"));
            } else {
                let (_, abs) = key_of(&ok(reply), E);
                let utc = ok(s.dispatch(&abs, "_get_utc_time", &[], E));
                let utc = read_utc(&mut Reader::new(&utc, E)).unwrap();
                assert_eq!((utc.time, utc.inaccuracy()), (1_000_000, 0));
            }
            // The other operations of the UTO read no clock: served.
            let (_, env) = key_of(&ok(s.dispatch(&zero, "interval", &[], E)), E);
            let iv = ok(s.dispatch(&env, "_get_time_interval", &[], E));
            assert_eq!(read_interval(&mut Reader::new(&iv, E)), Some((0, 0)));
        }
    }

    #[test]
    fn time_b_2_secure_universal_time_never_leaves_the_address_space() {
        use crate::admin::{AdminOp, AdminSubject, AuditError, AuditRecord, AuditSink};
        use crate::secure::{SecureTimeSource, SourceDesignation, SourceReading, SyncProtection};
        struct Accept;
        impl AuditSink for Accept {
            fn record(&self, _r: &AuditRecord) -> Result<(), AuditError> {
                Ok(())
            }
        }
        struct Synced;
        impl SecureTimeSource for Synced {
            fn read(&self) -> Result<SourceReading, crate::TimeUnavailable> {
                Ok(SourceReading {
                    time: 2_000_000,
                    inaccuracy: 10,
                    synchronized: true,
                })
            }
        }
        let (service, admin) = TimeService::with_source(|| 1_000_000).administered(
            alloc::sync::Arc::new(|_: &AdminSubject, _: AdminOp| true),
            alloc::sync::Arc::new(Accept),
        );
        admin
            .designate_source(
                &AdminSubject::new("ops"),
                SourceDesignation::new(
                    "ref",
                    SyncProtection::LocalReference,
                    alloc::sync::Arc::new(Synced),
                ),
            )
            .unwrap();
        // Secure in the address space of the service ...
        assert_eq!(service.secure_universal_time().unwrap().time(), 2_000_000);
        // ... never over GIOP, under either policy.
        for policy in [
            RemoteTimePolicy::Refuse,
            RemoteTimePolicy::AnswerCorrectingProxies,
        ] {
            let s =
                TimeServant::new(service.clone(), String::from("h"), 1).with_remote_time(policy);
            assert!(is_time_unavailable(s.dispatch(
                TIME_SERVICE_OBJECT_KEY,
                "secure_universal_time",
                &[],
                E
            )));
        }
    }

    #[test]
    fn time_1_2_new_universal_time_bad_param_and_utc_round_trip() {
        let s = servant();
        let mut w = Writer::new(Endianness::Big);
        w.write_u64(500);
        w.write_u64(crate::time_base::MAX_INACCURACY + 1);
        w.write_i16(0);
        assert_eq!(
            s.dispatch(
                TIME_SERVICE_OBJECT_KEY,
                "new_universal_time",
                &w.finish(),
                Endianness::Big
            ),
            ServantReply::SystemException("BAD_PARAM")
        );
        let mut w = Writer::new(Endianness::Big);
        w.write_u64(500);
        w.write_u64(0xAABB_CCDD_EEFF);
        w.write_i16(-300);
        let body = ok(s.dispatch(
            TIME_SERVICE_OBJECT_KEY,
            "new_universal_time",
            &w.finish(),
            Endianness::Big,
        ));
        let (_, key) = key_of(&body, Endianness::Big);
        let utc = ok(s.dispatch(&key, "_get_utc_time", &[], Endianness::Big));
        let utc = read_utc(&mut Reader::new(&utc, Endianness::Big)).unwrap();
        assert_eq!(utc, UtcT::new(500, 0xAABB_CCDD_EEFF, -300).unwrap());
        let inacc = ok(s.dispatch(&key, "_get_inaccuracy", &[], Endianness::Big));
        assert_eq!(
            Reader::new(&inacc, Endianness::Big).read_u64().unwrap(),
            0xAABB_CCDD_EEFF
        );
    }

    #[test]
    fn time_1_2_new_interval_bad_param_and_tio_operations() {
        let s = servant();
        let mut w = Writer::new(E);
        w.write_u64(200);
        w.write_u64(100);
        assert_eq!(
            s.dispatch(TIME_SERVICE_OBJECT_KEY, "new_interval", &w.finish(), E),
            ServantReply::SystemException("BAD_PARAM")
        );
        let mut w = Writer::new(E);
        w.write_u64(100);
        w.write_u64(300);
        let (type_id, a) = key_of(
            &ok(s.dispatch(TIME_SERVICE_OBJECT_KEY, "new_interval", &w.finish(), E)),
            E,
        );
        assert_eq!(type_id, TIO_TYPE_ID);
        // overlaps(in TIO, out TIO): [100,300] contains [150,250].
        let b = s
            .tio_ior(Tio::from_interval(IntervalT::new(150, 250).unwrap()))
            .unwrap();
        let reply = ok(s.dispatch(&a, "overlaps", &ior_body(&b, E), E));
        let mut r = Reader::new(&reply, E);
        assert_eq!(r.read_u32().unwrap(), 0); // OTContainer
        let out = Ior::decode(&mut r).unwrap();
        let out_key = iiop_of(&out).unwrap().object_key;
        let iv = ok(s.dispatch(&out_key, "_get_time_interval", &[], E));
        assert_eq!(read_interval(&mut Reader::new(&iv, E)), Some((150, 250)));
        // spans(in UTO, out TIO): UTO 400 ± 10 lies after [100,300].
        let uto = s.uto_ior(Uto::new(400, 10, 0).unwrap()).unwrap();
        let reply = ok(s.dispatch(&a, "spans", &ior_body(&uto, E), E));
        let mut r = Reader::new(&reply, E);
        assert_eq!(r.read_u32().unwrap(), 3); // OTNoOverlap
        // time(): midpoint 200, inaccuracy 100.
        let (_, mid) = key_of(&ok(s.dispatch(&a, "time", &[], E)), E);
        let utc = ok(s.dispatch(&mid, "_get_utc_time", &[], E));
        let utc = read_utc(&mut Reader::new(&utc, E)).unwrap();
        assert_eq!((utc.time, utc.inaccuracy()), (200, 100));
    }

    #[test]
    fn time_1_2_uto_operations_compare_interval_absolute() {
        let s = servant();
        let a = uto_object_key(UtcT::new(100, 10, 0).unwrap());
        let b = s.uto_ior(Uto::new(200, 10, 0).unwrap()).unwrap();
        let mut w = Writer::new(E);
        w.write_u32(1); // MidC
        b.encode(&mut w).unwrap();
        let reply = ok(s.dispatch(&a, "compare_time", &w.finish(), E));
        assert_eq!(Reader::new(&reply, E).read_u32().unwrap(), 1); // TCLessThan
        let mut w = Writer::new(E);
        w.write_u32(7);
        b.encode(&mut w).unwrap();
        assert_eq!(
            s.dispatch(&a, "compare_time", &w.finish(), E),
            ServantReply::SystemException("BAD_PARAM")
        );
        let (_, tio) = key_of(
            &ok(s.dispatch(&a, "time_to_interval", &ior_body(&b, E), E)),
            E,
        );
        let iv = ok(s.dispatch(&tio, "_get_time_interval", &[], E));
        assert_eq!(read_interval(&mut Reader::new(&iv, E)), Some((100, 200)));
        let (_, env) = key_of(&ok(s.dispatch(&a, "interval", &[], E)), E);
        let iv = ok(s.dispatch(&env, "_get_time_interval", &[], E));
        assert_eq!(read_interval(&mut Reader::new(&iv, E)), Some((90, 110)));
        // absolute_time on the service clock (1_000_000) with its tdf.
        let (_, abs) = key_of(&ok(s.dispatch(&a, "absolute_time", &[], E)), E);
        let utc = ok(s.dispatch(&abs, "_get_utc_time", &[], E));
        let utc = read_utc(&mut Reader::new(&utc, E)).unwrap();
        assert_eq!((utc.time, utc.tdf), (1_000_100, 60));
        let huge = uto_object_key(UtcT::new(u64::MAX, 0, 0).unwrap());
        assert_eq!(
            s.dispatch(&huge, "absolute_time", &[], E),
            ServantReply::SystemException("DATA_CONVERSION")
        );
    }

    #[test]
    fn time_1_2_object_operations_and_unknown_keys() {
        let s = servant();
        let mut w = Writer::new(E);
        w.write_string(TIME_SERVICE_TYPE_ID);
        let reply = ok(s.dispatch(TIME_SERVICE_OBJECT_KEY, "_is_a", &w.finish(), E));
        assert!(Reader::new(&reply, E).read_bool().unwrap());
        let mut w = Writer::new(E);
        w.write_string(UTO_TYPE_ID);
        let reply = ok(s.dispatch(TIME_SERVICE_OBJECT_KEY, "_is_a", &w.finish(), E));
        assert!(!Reader::new(&reply, E).read_bool().unwrap());
        assert_eq!(
            s.dispatch(b"Other", "universal_time", &[], E),
            ServantReply::SystemException("OBJECT_NOT_EXIST")
        );
        assert_eq!(
            s.dispatch(TIME_SERVICE_OBJECT_KEY, "no_such_op", &[], E),
            ServantReply::SystemException("BAD_OPERATION")
        );
        assert_eq!(
            s.dispatch(TIME_SERVICE_OBJECT_KEY, "new_interval", &[1, 2], E),
            ServantReply::SystemException("MARSHAL")
        );
        // A TIO key with lower > upper names no object.
        let mut bad = TIO_KEY_PREFIX.to_vec();
        bad.extend_from_slice(&9u64.to_be_bytes());
        bad.extend_from_slice(&1u64.to_be_bytes());
        assert!(!s.knows(&bad));
        assert!(s.knows(TIME_SERVICE_OBJECT_KEY));
    }

    #[test]
    fn time_1_2_giop_1_0_bodies_off_an_8_octet_boundary_are_aligned_from_the_message() {
        // A GIOP 1.0 request body that starts 4 octets off an 8-octet
        // boundary: the first `unsigned long long` follows 4 octets of
        // padding. The reply body starts 4 off as well.
        let s = servant();
        let mut w = Writer::new(E);
        for _ in 0..4 {
            w.write_u8(0);
        }
        w.write_u64(1_000);
        w.write_u64(50);
        w.write_i16(-60);
        let body = w.finish()[4..].to_vec();
        assert_eq!(&body[..4], &[0, 0, 0, 0]);
        let reply = ok(s.dispatch_at(
            TIME_SERVICE_OBJECT_KEY,
            "new_universal_time",
            &body,
            20,
            24,
            E,
        ));
        let (_, key) = key_of(&reply, E);
        let utc = ok(s.dispatch_at(&key, "_get_utc_time", &[], 0, 12, E));
        // The reply body starts at offset 12: 4 octets of padding first.
        let mut padded = alloc::vec![0u8; 4];
        padded.extend_from_slice(&utc);
        let mut r = Reader::new(&padded, E);
        r.read_bytes(4).unwrap();
        assert_eq!(&utc[..4], &[0, 0, 0, 0]);
        let utc = read_utc(&mut r).unwrap();
        assert_eq!((utc.time, utc.inaccuracy(), utc.tdf), (1_000, 50, -60));
    }

    #[test]
    fn time_1_3_5_2_a_nil_or_unreadable_uto_is_bad_param() {
        let s = servant();
        let a = tio_object_key(IntervalT::new(0, 10).unwrap());
        let nil = Ior {
            type_id: String::new(),
            profiles: Vec::new(),
        };
        assert_eq!(
            s.dispatch(&a, "spans", &ior_body(&nil, E), E),
            ServantReply::SystemException("BAD_PARAM")
        );
        // A reference to a port nothing listens on.
        let gone = TimeServant::new(TimeService::default(), String::from("127.0.0.1"), 1)
            .ior(b"elsewhere".to_vec(), UTO_TYPE_ID)
            .unwrap();
        assert_eq!(
            s.dispatch(&a, "spans", &ior_body(&gone, E), E),
            ServantReply::SystemException("BAD_PARAM")
        );
    }
}
