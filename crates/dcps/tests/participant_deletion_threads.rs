// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What is left running of a participant once `delete_participant` has
//! returned (DDS 1.4 §2.2.2.2.2.2: the operation "deletes an existing
//! DomainParticipant"): nothing. `DcpsRuntime::shutdown`, which the
//! deletion calls, documents that it stops the participant's threads and
//! joins them. One test in this binary, so that the threads of the process
//! are the threads of this test.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

#[path = "common/mod.rs"]
mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use zerodds_dcps::listener::DataWriterListener;
use zerodds_dcps::psm_constants::status as bits;
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::status::PublicationMatchedStatus;
use zerodds_dcps::*;

/// How long the writer's listener is busy with the match it hears.
const LISTENER_BUSY: Duration = Duration::from_millis(300);

/// Signals when it is called, is busy for [`LISTENER_BUSY`], and records
/// that it returned.
struct BusyListener {
    entered: std::sync::Mutex<Option<mpsc::Sender<()>>>,
    returned: AtomicBool,
}

impl DataWriterListener for BusyListener {
    fn on_publication_matched(&self, _w: InstanceHandle, _s: PublicationMatchedStatus) {
        let Some(entered) = self.entered.lock().unwrap().take() else {
            return;
        };
        let _ = entered.send(());
        std::thread::sleep(LISTENER_BUSY);
        self.returned.store(true, Ordering::SeqCst);
    }
}

/// The threads of this process by kernel task id, with their names.
#[cfg(target_os = "linux")]
fn threads() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir("/proc/self/task").expect("/proc/self/task") {
        let path = entry.expect("a task").path();
        let tid = path.file_name().unwrap().to_string_lossy().into_owned();
        let name = std::fs::read_to_string(path.join("comm")).unwrap_or_default();
        out.push((tid, name.trim().to_string()));
    }
    out
}

/// A participant that sends no multicast (`discovery.no_multicast`), with
/// a writer whose listener is busy when the participant is deleted:
///
/// - `delete_participant` returns after the listener has returned, and no
///   listener call follows: the listener thread is joined. It was told to
///   stop and not waited for, and a listener ran on after the deletion.
/// - On Linux every thread the participant started has ended, and the
///   deletion did not wait out a socket's 1 s read timeout. With no
///   multicast sent, the SPDP multicast receive thread got no wake-up
///   datagram, and every such shutdown took the second (50 tests of
///   `zerodds-dcps --features security`, 1.0 to 1.1 s each).
#[test]
fn dcps_2_2_2_2_2_2_a_deleted_participant_has_ended_its_threads() {
    let lease = common::lease_domain();
    #[cfg(target_os = "linux")]
    let before = threads();
    let cfg = RuntimeConfig {
        spdp_multicast_send: false,
        ..common::isolated_cfg()
    };
    let factory = DomainParticipantFactory::instance();
    let participant = factory
        .create_participant_with_config(lease.id(), DomainParticipantQos::default(), cfg)
        .expect("participant");
    let topic = participant
        .create_topic::<RawBytes>("DeletedParticipantThreads", TopicQos::default())
        .expect("topic");
    let publisher = participant.create_publisher(PublisherQos::default());
    let writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");
    let (entered_tx, entered_rx) = mpsc::channel();
    let listener = Arc::new(BusyListener {
        entered: std::sync::Mutex::new(Some(entered_tx)),
        returned: AtomicBool::new(false),
    });
    writer.set_listener(Some(listener.clone()), bits::PUBLICATION_MATCHED);
    let subscriber = participant.create_subscriber(SubscriberQos::default());
    let reader = subscriber
        .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
        .expect("reader");
    entered_rx
        .recv_timeout(common::match_timeout())
        .expect("the writer's listener hears the match");
    #[cfg(target_os = "linux")]
    let started: Vec<(String, String)> = threads()
        .into_iter()
        .filter(|thread| !before.contains(thread))
        .collect();

    participant
        .delete_contained_entities()
        .expect("delete_contained_entities");
    let deleting = std::time::Instant::now();
    factory
        .delete_participant(&participant)
        .expect("delete_participant");
    let took = deleting.elapsed();
    assert!(
        listener.returned.load(Ordering::SeqCst),
        "delete_participant returned while the listener was still busy"
    );

    #[cfg(target_os = "linux")]
    {
        assert!(
            started
                .iter()
                .any(|(_, name)| name.starts_with("zdds-recv-spdp")),
            "the participant's threads are among {started:?}"
        );
        // The listener is busy for 300 ms of it at most.
        assert!(
            took < Duration::from_millis(800),
            "delete_participant took {took:?}"
        );
        // A joined thread leaves the task list once the kernel has
        // released it, a moment after the join.
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        let left = loop {
            let now = threads();
            let left: Vec<&(String, String)> = started
                .iter()
                .filter(|thread| now.contains(thread))
                .collect();
            if left.is_empty() || std::time::Instant::now() > deadline {
                break left.into_iter().cloned().collect::<Vec<_>>();
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(
            left.is_empty(),
            "still running after the deletion: {left:?}"
        );
    }
    #[cfg(not(target_os = "linux"))]
    let _ = took;
    drop((reader, writer, subscriber, publisher, topic));
}
