// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A latency-correcting proxy of a remote `CosTime::TimeService` — OMG
//! Time Service 1.1 App. B.2. Feature `serve`.
//!
//! B.2: a timestamp is valid at the instant the Time Service object
//! returns it; a proxy that delivers it later makes its uncertainty
//! interval wrong. "Implementors of the Time Service must prevent this
//! problem from occurring", for example with "a special time server
//! proxy, which measures latency between the Time Service object and the
//! proxy, recalculates the time interval's uncertainty, and adjusts the
//! interval value before returning the timestamp to the caller".
//!
//! [`TimeServiceProxy::universal_time`] is that proxy. It reads the
//! monotonic clock of its own process before it sends `universal_time`
//! and after the reply of `_get_utc_time` on the returned UTO. The
//! service generated the UTO between the two readings, so at the second
//! reading the true time lies within `[time - inaccuracy, time +
//! inaccuracy + d]`, `d` the elapsed time. The proxy returns the
//! midpoint and half width of that interval: `time + ⌈d/2⌉` and
//! `inaccuracy + ⌈d/2⌉` (Cristian, "Probabilistic Clock Synchronization",
//! App. F). [`TimeServiceProxy::absolute_time`] does the same for the
//! other operation that reads the clock of the service, `absolute_time`
//! of a UTO (§1.3.4.5). The server must answer both:
//! [`crate::servant::RemoteTimePolicy::AnswerCorrectingProxies`].
//!
//! [`TimeServiceProxy::secure_universal_time`] raises `TimeUnavailable`
//! without a call: the time would cross IIOP without protection
//! (App. B.1.1), and the server never sends secure time.

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use zerodds_corba_giop::{
    Message, ReplyStatusType, Request, ResponseFlags, TargetAddress, Version,
};
use zerodds_corba_iiop::{Connector, ConnectorConfig, IiopProfileBody, IiopVersion};
use zerodds_corba_ior::{Ior, TaggedProfile};
use zerodds_wire::{Endianness, Reader, Writer};

use crate::exception::AbsoluteTimeError;
use crate::servant::TIME_SERVICE_OBJECT_KEY;
use crate::service::TimeUnavailable;
use crate::time_base::UtcT;
use crate::uto::Uto;

/// How long one call of the proxy may take on the network.
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// `elapsed` in 100 ns ticks, rounded up.
fn ticks_up(elapsed: Duration) -> u64 {
    let nanos = elapsed.as_nanos();
    u64::try_from(nanos.div_ceil(100)).unwrap_or(u64::MAX)
}

/// Why a latency correction gave no UTO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Uncorrectable {
    /// The corrected time does not fit `TimeT`.
    Time,
    /// The widened inaccuracy does not fit 48 bits.
    Inaccuracy,
}

/// Time and inaccuracy of `utc` each plus half of `elapsed`, rounded up.
fn widen(utc: UtcT, elapsed: Duration) -> Result<Uto, Uncorrectable> {
    let half = ticks_up(elapsed).div_ceil(2);
    let time = utc.time.checked_add(half).ok_or(Uncorrectable::Time)?;
    let inaccuracy = utc
        .inaccuracy()
        .checked_add(half)
        .ok_or(Uncorrectable::Inaccuracy)?;
    Uto::new(time, inaccuracy, utc.tdf).map_err(|_| Uncorrectable::Inaccuracy)
}

/// The UTO the proxy returns for the value `utc` the service generated
/// within the last `elapsed`: time and inaccuracy each plus half of
/// `elapsed`, rounded up; the tdf of the service.
///
/// # Errors
/// [`TimeUnavailable`] if the widened inaccuracy does not fit 48 bits or
/// the time does not fit `TimeT`.
pub fn correct_for_latency(utc: UtcT, elapsed: Duration) -> Result<Uto, TimeUnavailable> {
    widen(utc, elapsed).map_err(|_| TimeUnavailable)
}

/// [`correct_for_latency`] for the result of `absolute_time`: a corrected
/// time beyond `TimeT` is the overflow §1.3.4.5 names
/// ([`AbsoluteTimeError::DataConversion`]), an inaccuracy beyond 48 bits
/// [`AbsoluteTimeError::TimeUnavailable`].
fn correct_absolute(utc: UtcT, elapsed: Duration) -> Result<Uto, AbsoluteTimeError> {
    widen(utc, elapsed).map_err(|u| match u {
        Uncorrectable::Time => AbsoluteTimeError::DataConversion,
        Uncorrectable::Inaccuracy => AbsoluteTimeError::TimeUnavailable,
    })
}

/// The byte order of the requests the proxy sends and of their bodies.
const REQUEST_ENDIANNESS: Endianness = Endianness::Little;

/// Repository id of the system exception `CORBA::DATA_CONVERSION`.
const DATA_CONVERSION_ID: &str = "IDL:omg.org/CORBA/DATA_CONVERSION:1.0";

/// The reply to one request.
struct Answer {
    status: ReplyStatusType,
    body: Vec<u8>,
    endianness: Endianness,
}

/// The first IIOP profile of the IOR a reply body carries.
fn returned_object(
    body: &[u8],
    endianness: Endianness,
) -> Result<IiopProfileBody, TimeUnavailable> {
    let ior = Ior::decode(&mut Reader::new(body, endianness)).map_err(|_| TimeUnavailable)?;
    ior.profiles
        .iter()
        .find_map(TaggedProfile::as_iiop)
        .and_then(Result::ok)
        .ok_or(TimeUnavailable)
}

/// The `UtcT` a reply body of `_get_utc_time` carries.
fn returned_utc(body: &[u8], endianness: Endianness) -> Result<UtcT, TimeUnavailable> {
    let mut r = Reader::new(body, endianness);
    Ok(UtcT {
        time: r.read_u64().map_err(|_| TimeUnavailable)?,
        inacclo: r.read_u32().map_err(|_| TimeUnavailable)?,
        inacchi: r.read_u16().map_err(|_| TimeUnavailable)?,
        tdf: r.read_i16().map_err(|_| TimeUnavailable)?,
    })
}

/// A failed call or an unreadable reply of `absolute_time`.
const fn unavailable(_: TimeUnavailable) -> AbsoluteTimeError {
    AbsoluteTimeError::TimeUnavailable
}

/// A proxy of the `CosTime::TimeService` at an IIOP address.
pub struct TimeServiceProxy {
    host: String,
    port: u16,
    object_key: Vec<u8>,
    connector: Connector,
    next_request_id: AtomicU32,
}

impl core::fmt::Debug for TimeServiceProxy {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TimeServiceProxy")
            .field("host", &self.host)
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

impl TimeServiceProxy {
    /// The service at `corbaloc::<host>:<port>/TimeService`.
    #[must_use]
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self::with_key(host.into(), port, TIME_SERVICE_OBJECT_KEY.to_vec())
    }

    /// The service an IOR names (its first IIOP profile); `None` without
    /// one.
    #[must_use]
    pub fn from_ior(ior: &Ior) -> Option<Self> {
        let profile = ior
            .profiles
            .iter()
            .find_map(TaggedProfile::as_iiop)
            .and_then(Result::ok)?;
        Some(Self::with_key(
            profile.host,
            profile.port,
            profile.object_key,
        ))
    }

    fn with_key(host: String, port: u16, object_key: Vec<u8>) -> Self {
        let config = ConnectorConfig {
            connect_timeout: Some(CALL_TIMEOUT),
            read_timeout: Some(CALL_TIMEOUT),
            write_timeout: Some(CALL_TIMEOUT),
            ..ConnectorConfig::default()
        };
        Self {
            host,
            port,
            object_key,
            connector: Connector::new(config),
            next_request_id: AtomicU32::new(1),
        }
    }

    /// One GIOP 1.2 request with the arguments `body` (written in
    /// [`REQUEST_ENDIANNESS`]) and its reply; [`TimeUnavailable`] for no
    /// connection or a timeout.
    fn call(
        &self,
        target: &IiopProfileBody,
        operation: &str,
        body: Vec<u8>,
    ) -> Result<Answer, TimeUnavailable> {
        let mut pooled = self
            .connector
            .connect(&target.host, target.port)
            .map_err(|_| TimeUnavailable)?;
        let conn = pooled.connection().ok_or(TimeUnavailable)?;
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let mut request = Request::new(
            request_id,
            ResponseFlags::SYNC_WITH_TARGET,
            TargetAddress::Key(target.object_key.clone()),
            String::from(operation),
        );
        request.body = body;
        conn.write_message(
            Version::V1_2,
            REQUEST_ENDIANNESS,
            false,
            &Message::Request(request),
        )
        .map_err(|_| TimeUnavailable)?;
        loop {
            let (msg, endianness, _) = conn.read_message_full().map_err(|_| TimeUnavailable)?;
            if let Message::Reply(reply) = msg {
                if reply.request_id != request_id {
                    continue;
                }
                return Ok(Answer {
                    status: reply.reply_status,
                    body: reply.body,
                    endianness,
                });
            }
        }
    }

    /// [`Self::call`]; the reply body if it is `NO_EXCEPTION`, else
    /// [`TimeUnavailable`].
    fn call_ok(
        &self,
        target: &IiopProfileBody,
        operation: &str,
        body: Vec<u8>,
    ) -> Result<(Vec<u8>, Endianness), TimeUnavailable> {
        let answer = self.call(target, operation, body)?;
        if answer.status != ReplyStatusType::NoException {
            return Err(TimeUnavailable);
        }
        Ok((answer.body, answer.endianness))
    }

    /// The service object.
    fn service(&self) -> IiopProfileBody {
        IiopProfileBody::new(
            IiopVersion::V1_2,
            self.host.clone(),
            self.port,
            self.object_key.clone(),
        )
    }

    /// `universal_time` of the remote service, corrected for the latency
    /// of the two calls ([`correct_for_latency`]).
    ///
    /// # Errors
    /// [`TimeUnavailable`] if the service raises it or any other
    /// exception (`NO_PERMISSION` when it refuses remote current time),
    /// if a call fails, or if the corrected inaccuracy does not fit
    /// 48 bits.
    pub fn universal_time(&self) -> Result<Uto, TimeUnavailable> {
        let start = Instant::now();
        let (body, endianness) = self.call_ok(&self.service(), "universal_time", Vec::new())?;
        let uto = returned_object(&body, endianness)?;
        let (body, endianness) = self.call_ok(&uto, "_get_utc_time", Vec::new())?;
        let elapsed = start.elapsed();
        let utc = returned_utc(&body, endianness)?;
        if utc.time == 0 {
            return Err(TimeUnavailable);
        }
        correct_for_latency(utc, elapsed)
    }

    /// `absolute_time` of the UTO `relative` on the remote service
    /// (§1.3.4.5: the current time of the service plus the time in the
    /// object), corrected for the latency as [`Self::universal_time`]
    /// is. The proxy makes `relative` an object of the service with
    /// `uto_from_utc` (no clock is read), then measures the round trip of
    /// `absolute_time` on it and `_get_utc_time` on the result.
    ///
    /// # Errors
    /// [`AbsoluteTimeError::DataConversion`] if the service raises
    /// `CORBA::DATA_CONVERSION` (the sum overflows `TimeT`) or the
    /// corrected time does not fit `TimeT`;
    /// [`AbsoluteTimeError::TimeUnavailable`] if the service raises any
    /// other exception (`NO_PERMISSION` when it refuses remote current
    /// time, `TRANSIENT` when its clock gives no time), if a call fails,
    /// or if the corrected inaccuracy does not fit 48 bits.
    pub fn absolute_time(&self, relative: Uto) -> Result<Uto, AbsoluteTimeError> {
        let utc = relative.utc_time();
        let mut w = Writer::new(REQUEST_ENDIANNESS);
        w.write_u64(utc.time);
        w.write_u32(utc.inacclo);
        w.write_u16(utc.inacchi);
        w.write_i16(utc.tdf);
        let (body, endianness) = self
            .call_ok(&self.service(), "uto_from_utc", w.finish())
            .map_err(unavailable)?;
        let object = returned_object(&body, endianness).map_err(unavailable)?;
        let start = Instant::now();
        let answer = self
            .call(&object, "absolute_time", Vec::new())
            .map_err(unavailable)?;
        match answer.status {
            ReplyStatusType::NoException => {}
            ReplyStatusType::SystemException
                if Reader::new(&answer.body, answer.endianness)
                    .read_string()
                    .is_ok_and(|id| id == DATA_CONVERSION_ID) =>
            {
                return Err(AbsoluteTimeError::DataConversion);
            }
            _ => return Err(AbsoluteTimeError::TimeUnavailable),
        }
        let absolute = returned_object(&answer.body, answer.endianness).map_err(unavailable)?;
        let (body, endianness) = self
            .call_ok(&absolute, "_get_utc_time", Vec::new())
            .map_err(unavailable)?;
        let elapsed = start.elapsed();
        correct_absolute(
            returned_utc(&body, endianness).map_err(unavailable)?,
            elapsed,
        )
    }

    /// Always [`TimeUnavailable`], without a call: the time would cross
    /// IIOP without protection (App. B.1.1), and the server never sends
    /// secure time.
    ///
    /// # Errors
    /// Always [`TimeUnavailable`].
    pub fn secure_universal_time(&self) -> Result<Uto, TimeUnavailable> {
        Err(TimeUnavailable)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::time_base::MAX_INACCURACY;

    #[test]
    fn time_b_2_correction_widens_by_half_the_round_trip_each_side() {
        let utc = UtcT::new(1_000_000, 40, -60).unwrap();
        // 1 ms = 10 000 ticks: midpoint and half width move by 5 000.
        let uto = correct_for_latency(utc, Duration::from_millis(1)).unwrap();
        assert_eq!(uto.time(), 1_005_000);
        assert_eq!(uto.inaccuracy(), 5_040);
        assert_eq!(uto.tdf(), -60);
        // The interval covers [time - inaccuracy, time + inaccuracy + d].
        assert_eq!(uto.time() - uto.inaccuracy(), 1_000_000 - 40);
        assert!(uto.time() + uto.inaccuracy() >= 1_000_000 + 40 + 10_000);
        // 250 ns: 3 ticks rounded up, half of that rounded up.
        let uto = correct_for_latency(utc, Duration::from_nanos(250)).unwrap();
        assert_eq!((uto.time(), uto.inaccuracy()), (1_000_002, 42));
        let uto = correct_for_latency(utc, Duration::ZERO).unwrap();
        assert_eq!((uto.time(), uto.inaccuracy()), (1_000_000, 40));
    }

    #[test]
    fn time_b_2_correction_beyond_48_bits_is_time_unavailable() {
        let utc = UtcT::new(1_000, MAX_INACCURACY - 1, 0).unwrap();
        assert!(correct_for_latency(utc, Duration::from_nanos(200)).is_ok());
        assert_eq!(
            correct_for_latency(utc, Duration::from_nanos(500)),
            Err(TimeUnavailable)
        );
        let late = UtcT::new(u64::MAX, 0, 0).unwrap();
        assert_eq!(
            correct_for_latency(late, Duration::from_micros(1)),
            Err(TimeUnavailable)
        );
    }

    #[test]
    fn time_b_2_absolute_time_correction_overflow_is_data_conversion() {
        // §1.3.4.5: an absolute time beyond TimeT is DATA_CONVERSION; an
        // inaccuracy the widening pushes beyond 48 bits is TimeUnavailable.
        let late = UtcT::new(u64::MAX - 1, 0, 0).unwrap();
        assert_eq!(
            correct_absolute(late, Duration::from_nanos(100)).map(Uto::time),
            Ok(u64::MAX)
        );
        assert_eq!(
            correct_absolute(late, Duration::from_nanos(500)),
            Err(AbsoluteTimeError::DataConversion)
        );
        let wide = UtcT::new(1_000, MAX_INACCURACY - 1, 0).unwrap();
        assert_eq!(
            correct_absolute(wide, Duration::from_nanos(500)),
            Err(AbsoluteTimeError::TimeUnavailable)
        );
        let utc = UtcT::new(1_000_000, 40, -60).unwrap();
        let uto = correct_absolute(utc, Duration::from_millis(1)).unwrap();
        assert_eq!(
            (uto.time(), uto.inaccuracy(), uto.tdf()),
            (1_005_000, 5_040, -60)
        );
    }
}
