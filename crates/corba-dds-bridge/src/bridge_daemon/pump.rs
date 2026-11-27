// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The DDS side: the samples of the reply and notify topics (spec §4.3, §4.4).

#[allow(unused_imports)]
use super::prelude::*;

/// Whether a sample's body is plain XCDR1, the GIOP body of §4.3 and §4.4:
/// the representation identifier `CDR_BE` or `CDR_LE` (DDS-XTypes 1.3
/// §7.6.3.1.2, Table 60). `PL_CDR` (an `@mutable` type in XCDR1) carries a
/// parameter list, and XCDR2 aligns 8-octet members at 4; neither is the
/// body a GIOP 1.2 peer reads.
pub(crate) fn plain_cdr(identifier: [u8; 2]) -> bool {
    identifier[0] == 0 && identifier[1] <= 0x01
}

/// The log text for a sample [`plain_cdr`] refuses.
pub(crate) fn not_plain_cdr(identifier: [u8; 2]) -> String {
    format!(
        "representation identifier 0x{:02x}{:02x}; the GIOP body is plain CDR",
        identifier[0], identifier[1]
    )
}

/// The reply pump of mapping `index`: each sample of its reply topic goes
/// to [`on_reply`] (§4.3 step 7).
pub(crate) fn pump_replies(d: &Dispatch, index: usize, rx: &mpsc::Receiver<UserSample>) {
    while let Ok(s) = rx.recv() {
        if let UserSample::Alive {
            payload,
            big_endian,
            representation_identifier,
            ..
        } = s
        {
            on_reply(
                d,
                index,
                payload.as_ref(),
                big_endian,
                representation_identifier,
            );
        }
    }
}

/// The notifiers of a mapping's targets (§4.4).
pub(crate) fn notifiers_of(m: &Mapping) -> Vec<Notifier> {
    m.targets
        .iter()
        .filter_map(|t| resolve_target(t).ok())
        .map(|target| Notifier {
            target,
            operation: m.operation.clone(),
            conn: None,
            next_request_id: 1,
            resolved: false,
        })
        .collect()
}

/// The notify pump of mapping `index`: each sample of its notify topic goes
/// to [`on_notify_sample`].
pub(crate) fn pump_notify(d: &Dispatch, index: usize, rx: &mpsc::Receiver<UserSample>) {
    let mut notifiers = notifiers_of(&d.cfg.mappings[index]);
    while let Ok(s) = rx.recv() {
        if let UserSample::Alive {
            payload,
            big_endian,
            representation_identifier,
            ..
        } = s
        {
            on_notify_sample(
                d,
                index,
                &mut notifiers,
                payload.as_ref(),
                big_endian,
                representation_identifier,
            );
        }
    }
}

/// One sample of mapping `index`'s notify topic (§4.4): a sample that is not
/// plain CDR is refused and counted, any other goes as a oneway request to
/// every target. Returns how many targets took it.
pub(crate) fn on_notify_sample(
    d: &Dispatch,
    index: usize,
    notifiers: &mut [Notifier],
    payload: &[u8],
    big_endian: bool,
    identifier: [u8; 2],
) -> usize {
    note_sample(d, index);
    let m = &d.cfg.mappings[index];
    if !plain_cdr(identifier) {
        d.metrics.errors_total.inc();
        log(
            "notify_sample_refused",
            &[
                ("topic", &m.notify_topic.dds_name),
                ("err", &not_plain_cdr(identifier)),
            ],
        );
        return 0;
    }
    d.metrics.dds_samples_in_total.inc();
    if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(index)) {
        om.received.inc();
    }
    let e = if big_endian {
        Endianness::Big
    } else {
        Endianness::Little
    };
    let mut sent = 0;
    for n in notifiers.iter_mut() {
        match n.send(payload, e) {
            Ok(()) => {
                sent += 1;
                d.metrics.frames_out_total.inc();
                log(
                    "notify",
                    &[("target", &format!("{}:{}", n.target.host, n.target.port))],
                );
            }
            Err(err) => {
                d.metrics.errors_total.inc();
                log(
                    "notify_failed",
                    &[
                        ("target", &format!("{}:{}", n.target.host, n.target.port)),
                        ("err", &err.to_string()),
                    ],
                );
            }
        }
    }
    sent
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::bridge_daemon::server::tests::fixture;

    /// A fake target: reads the GIOP messages of one connection (it answers
    /// no `LocateRequest`, as JacORB and omniORB may) and hands back the
    /// oneway requests.
    fn target() -> (u16, std::thread::JoinHandle<Vec<Request>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            let mut c = Connection::from_stream(s).unwrap();
            c.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut out = Vec::new();
            while let Ok(m) = c.read_message() {
                if let Message::Request(r) = m {
                    out.push(r);
                    break;
                }
            }
            out
        });
        (port, h)
    }

    fn notify_yaml(ports: &[u16]) -> String {
        let targets: Vec<String> = ports
            .iter()
            .map(|p| format!("\"corbaloc:iiop:1.2@127.0.0.1:{p}/Listener\""))
            .collect();
        format!(
            "mappings:\n  - repo_id: \"IDL:x/Listener:1.0\"\n    operation: on_value\n    direction: dds_to_corba\n    targets: [{}]\n",
            targets.join(", ")
        )
    }

    /// §4.4: a plain CDR sample goes to the target as a oneway `on_value`
    /// with the sample as its body.
    #[test]
    fn corbabridge_4_4_a_sample_is_a_oneway_request_with_the_sample_as_body() {
        let (port, h) = target();
        let (d, _lease) = fixture(&notify_yaml(&[port]));
        let mut n = notifiers_of(&d.cfg.mappings[0]);
        assert_eq!(
            on_notify_sample(&d, 0, &mut n, &[7, 0, 0, 0], false, [0, 1]),
            1
        );
        let got = h.join().unwrap();
        assert_eq!(got[0].operation, "on_value");
        assert_eq!(got[0].body, [7, 0, 0, 0]);
        assert!(!got[0].response_flags.response_expected());
    }

    /// A sample that is not plain CDR does not go out.
    #[test]
    fn corbabridge_4_4_a_sample_that_is_not_plain_cdr_is_refused() {
        let (d, _lease) = fixture(&notify_yaml(&[1]));
        let mut n = notifiers_of(&d.cfg.mappings[0]);
        let before = d.metrics.errors_total.get();
        assert_eq!(
            on_notify_sample(&d, 0, &mut n, &[7, 0, 0, 0], false, [0, 3]),
            0
        );
        assert_eq!(d.metrics.errors_total.get(), before + 1);
    }

    /// Every target gets the sample.
    #[test]
    fn corbabridge_4_4_every_target_gets_the_sample() {
        let (a, ha) = target();
        let (b, hb) = target();
        let (d, _lease) = fixture(&notify_yaml(&[a, b]));
        let mut n = notifiers_of(&d.cfg.mappings[0]);
        assert_eq!(
            on_notify_sample(&d, 0, &mut n, &[1, 2, 3, 4], true, [0, 0]),
            2
        );
        assert_eq!(ha.join().unwrap()[0].body, [1, 2, 3, 4]);
        assert_eq!(hb.join().unwrap()[0].body, [1, 2, 3, 4]);
    }

    /// A target that cannot be reached is counted; the others still get the
    /// sample.
    #[test]
    fn corbabridge_4_4_an_unreachable_target_does_not_stop_the_others() {
        let dead = zerodds_test_domains::free_tcp_port();
        let (b, hb) = target();
        let (d, _lease) = fixture(&notify_yaml(&[dead, b]));
        let mut n = notifiers_of(&d.cfg.mappings[0]);
        let before = d.metrics.errors_total.get();
        assert_eq!(
            on_notify_sample(&d, 0, &mut n, &[5, 0, 0, 0], false, [0, 1]),
            1
        );
        assert_eq!(d.metrics.errors_total.get(), before + 1);
        assert_eq!(hb.join().unwrap()[0].body, [5, 0, 0, 0]);
    }

    /// §6: a sample of a notify topic restarts its deadline.
    #[test]
    fn corbabridge_6_a_notify_sample_restarts_the_deadline() {
        let (d, _lease) = fixture(&notify_yaml(&[1]));
        assert!(d.clocks[0].lock().unwrap().last.is_none());
        let mut n: Vec<Notifier> = Vec::new();
        on_notify_sample(&d, 0, &mut n, &[1, 0, 0, 0], false, [0, 1]);
        assert!(d.clocks[0].lock().unwrap().last.is_some());
    }
}
