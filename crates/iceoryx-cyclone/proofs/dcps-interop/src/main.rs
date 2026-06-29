//! Proof: a normal ZeroDDS typed DataWriter/DataReader, once enable_cyclone_iox-ed,
//! interoperates with Cyclone DDS over the iceoryx C++ (POSH) transport.
#[allow(non_snake_case)]
mod kmsg;
use kmsg::IoxOracle::KMsg;
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};
use zerodds_dcps::{TopicQos, PublisherQos, SubscriberQos, DataWriterQos, DataReaderQos};

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let factory = DomainParticipantFactory::instance();
    // `write` (offline): publish over iceoryx only — the writer is not RTPS-
    // discovered, so a Cyclone reader needs ALLOW_NONDISCOVERED_WRITERS (it
    // accepts RAW but drops serialized). `writeonline`: a real online
    // participant on domain 0, so Cyclone discovers the writer over RTPS SEDP
    // and accepts its serialized iceoryx sample (GUID-matched). `read`: offline.
    let online = mode == "writeonline";
    let p = if online {
        factory
            .create_participant(0, DomainParticipantQos::default())
            .expect("create_participant (online)")
    } else {
        factory.create_participant_offline(48, DomainParticipantQos::default())
    };
    let topic = p.create_topic::<KMsg>("KMsgTopic", TopicQos::default()).unwrap();

    if mode == "write" || mode == "writeonline" {
        let pubr = p.create_publisher(PublisherQos::default());
        let dw = pubr.create_datawriter::<KMsg>(&topic, DataWriterQos::default()).unwrap();
        dw.enable_cyclone_iox().expect("enable_cyclone_iox");
        let s = KMsg { id: 7, name: "hello-from-zerodds".to_string(), value: 0xABCD };
        println!(
            "ZeroDDS DataWriter<KMsg>::write id=7 value=0xABCD ({}), guid={:?}",
            if online { "online RTPS + cyclone-iox" } else { "offline cyclone-iox" },
            dw.rtps_guid()
        );
        // Give RTPS discovery a moment to match with Cyclone before publishing.
        if online { std::thread::sleep(std::time::Duration::from_secs(2)); }
        for _ in 0..300 { dw.write(&s).expect("write"); std::thread::sleep(std::time::Duration::from_millis(50)); }
    } else {
        let sub = p.create_subscriber(SubscriberQos::default());
        let dr = sub.create_datareader::<KMsg>(&topic, DataReaderQos::default()).unwrap();
        dr.enable_cyclone_iox().expect("enable_cyclone_iox");
        println!("ZeroDDS DataReader<KMsg>::take waiting (cyclone-iox)...");
        for _ in 0..100 {
            let v = dr.take().unwrap_or_default();
            for m in &v {
                println!("ZeroDDS GOT KMsg id={} name={:?} value=0x{:X}", m.id, m.name, m.value);
                if m.id == 7 && m.name == "hello-cyclone" && m.value == 0xABCD {
                    println!("READER OK: ZeroDDS DataReader read the Cyclone KMsg over iceoryx SHM");
                    return;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        eprintln!("READER FAIL");
        std::process::exit(1);
    }
}
