#![no_main]
//! Fuzz target: FragmentAssembler::insert. Input bytes are mapped into
//! arbitrary DataFragSubmessage values; the assembler must
//! enforce DoS caps and must never panic.

use libfuzzer_sys::fuzz_target;

use zerodds_rtps::fragment_assembler::FragmentAssembler;
use zerodds_rtps::submessages::DataFragSubmessage;
use zerodds_rtps::wire_types::{EntityId, FragmentNumber, SequenceNumber};

fn read_u32(data: &[u8], idx: usize) -> u32 {
    if data.len() < idx + 4 {
        return 0;
    }
    let mut b = [0u8; 4];
    b.copy_from_slice(&data[idx..idx + 4]);
    u32::from_le_bytes(b)
}

fn read_u16(data: &[u8], idx: usize) -> u16 {
    if data.len() < idx + 2 {
        return 0;
    }
    let mut b = [0u8; 2];
    b.copy_from_slice(&data[idx..idx + 2]);
    u16::from_le_bytes(b)
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 20 {
        return;
    }
    let writer_sn = SequenceNumber(i64::from(read_u32(data, 0) % 10_000));
    let frag_start = FragmentNumber(read_u32(data, 4) % 200);
    let count = (read_u16(data, 8) % 16).max(1);
    let frag_size = read_u16(data, 10).max(1);
    let sample_size = read_u32(data, 12) % 200_000;
    let payload_len = (read_u32(data, 16) as usize) % 2048;
    let payload = if data.len() > 20 {
        data[20..].iter().copied().take(payload_len).collect()
    } else {
        Vec::new()
    };

    let df = DataFragSubmessage {
        extra_flags: 0,
        reader_id: EntityId::user_reader_with_key([1, 2, 3]),
        writer_id: EntityId::user_writer_with_key([4, 5, 6]),
        writer_sn,
        fragment_starting_num: frag_start,
        fragments_in_submessage: count,
        fragment_size,
        sample_size,
        serialized_payload: payload.into(),
        inline_qos_flag: false,
        hash_key_flag: false,
        key_flag: false,
        non_standard_flag: false,
    };

    let mut a = FragmentAssembler::default();
    let _ = a.insert(&df);
});
