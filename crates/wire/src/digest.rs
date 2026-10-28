// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The digest the KeyHash uses.
//!
//! Its own copy rather than a dependency on the frontend: a runtime that
//! reached into the frontend for a hash would be a runtime that cannot
//! ship without it.

/// MD5, as DDSI-RTPS specifies it for the KeyHash.
///
/// Not a security primitive here and not used as one: the specification
/// names MD5 for this, and a different digest would produce hashes that
/// no other vendor agrees with. DDSI-RTPS 2.5 §9.6.3.8.
#[allow(clippy::many_single_char_names)]
pub fn md5(input: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76a_a478,
        0xe8c7_b756,
        0x2420_70db,
        0xc1bd_ceee,
        0xf57c_0faf,
        0x4787_c62a,
        0xa830_4613,
        0xfd46_9501,
        0x6980_98d8,
        0x8b44_f7af,
        0xffff_5bb1,
        0x895c_d7be,
        0x6b90_1122,
        0xfd98_7193,
        0xa679_438e,
        0x49b4_0821,
        0xf61e_2562,
        0xc040_b340,
        0x265e_5a51,
        0xe9b6_c7aa,
        0xd62f_105d,
        0x0244_1453,
        0xd8a1_e681,
        0xe7d3_fbc8,
        0x21e1_cde6,
        0xc337_07d6,
        0xf4d5_0d87,
        0x455a_14ed,
        0xa9e3_e905,
        0xfcef_a3f8,
        0x676f_02d9,
        0x8d2a_4c8a,
        0xfffa_3942,
        0x8771_f681,
        0x6d9d_6122,
        0xfde5_380c,
        0xa4be_ea44,
        0x4bde_cfa9,
        0xf6bb_4b60,
        0xbebf_bc70,
        0x289b_7ec6,
        0xeaa1_27fa,
        0xd4ef_3085,
        0x0488_1d05,
        0xd9d4_d039,
        0xe6db_99e5,
        0x1fa2_7cf8,
        0xc4ac_5665,
        0xf429_2244,
        0x432a_ff97,
        0xab94_23a7,
        0xfc93_a039,
        0x655b_59c3,
        0x8f0c_cc92,
        0xffef_f47d,
        0x8584_5dd1,
        0x6fa8_7e4f,
        0xfe2c_e6e0,
        0xa301_4314,
        0x4e08_11a1,
        0xf753_7e82,
        0xbd3a_f235,
        0x2ad7_d2bb,
        0xeb86_d391,
    ];

    let mut state = [0x6745_2301_u32, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    let mut blocks = input.chunks_exact(64);
    for block in &mut blocks {
        compress(&mut state, block, &S, &K);
    }
    // RFC 1321 §3.1–3.2: the rest of the input, one `0x80`, zeros, and the
    // bit length in the last eight octets — one block when the rest leaves
    // room for the length, two when it does not. On the stack: the KeyHash
    // is computed for every write of a keyed sample, and the copy of the
    // whole input this used to make was a heap allocation per write.
    let rest = blocks.remainder();
    let mut tail = [0u8; 128];
    tail[..rest.len()].copy_from_slice(rest);
    tail[rest.len()] = 0x80;
    let tail_len = if rest.len() < 56 { 64 } else { 128 };
    let bit_length = (input.len() as u64).wrapping_mul(8);
    tail[tail_len - 8..tail_len].copy_from_slice(&bit_length.to_le_bytes());
    for block in tail[..tail_len].chunks_exact(64) {
        compress(&mut state, block, &S, &K);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&state[0].to_le_bytes());
    out[4..8].copy_from_slice(&state[1].to_le_bytes());
    out[8..12].copy_from_slice(&state[2].to_le_bytes());
    out[12..16].copy_from_slice(&state[3].to_le_bytes());
    out
}

/// One 64-octet block into the state (RFC 1321 §3.4).
///
/// The four rounds as four loops, so the round function and the message
/// index are fixed per loop rather than chosen by a `match` in each of the
/// 64 steps.
fn compress(state: &mut [u32; 4], block: &[u8], s: &[u32; 64], k: &[u32; 64]) {
    let mut words = [0u32; 16];
    for (index, word) in words.iter_mut().enumerate() {
        let at = index * 4;
        *word = u32::from_le_bytes([block[at], block[at + 1], block[at + 2], block[at + 3]]);
    }
    let mut v = *state;
    for i in 0..16 {
        let [_, b, c, d] = v;
        step(&mut v, (b & c) | (!b & d), words[i], k[i], s[i]);
    }
    for i in 16..32 {
        let [_, b, c, d] = v;
        step(
            &mut v,
            (d & b) | (!d & c),
            words[(5 * i + 1) % 16],
            k[i],
            s[i],
        );
    }
    for i in 32..48 {
        let [_, b, c, d] = v;
        step(&mut v, b ^ c ^ d, words[(3 * i + 5) % 16], k[i], s[i]);
    }
    for i in 48..64 {
        let [_, b, c, d] = v;
        step(&mut v, c ^ (b | !d), words[(7 * i) % 16], k[i], s[i]);
    }
    for (word, add) in state.iter_mut().zip(v) {
        *word = word.wrapping_add(add);
    }
}

/// One step: `(a, b, c, d)` becomes `(d, b + ((a + f + k + word) <<< s), b, c)`.
#[inline(always)]
fn step(v: &mut [u32; 4], f: u32, word: u32, k: u32, s: u32) {
    let [a, b, c, d] = *v;
    let sum = a.wrapping_add(f).wrapping_add(k).wrapping_add(word);
    *v = [d, b.wrapping_add(sum.rotate_left(s)), b, c];
}
