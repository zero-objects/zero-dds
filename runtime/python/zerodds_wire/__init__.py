# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""`zerodds_wire` — the Python wire runtime.

The runtime a generated Python encoder calls: `Writer` and `Reader`, the
alignment rules, and the framing forms. It is the counterpart of
`crates/wire`, not a binding to it — a Python probe that reached the Rust
core through ctypes would be byte-identical by construction and would
prove nothing about whether the two read the specification the same way.

This package makes no decision about a type. Which members are written,
in what order, with which header and which length code is the layout
plan's answer and arrives here as arguments. A runtime that decided any
of it would be a second place where the wire format lives.

Spec: DDS-XTypes 1.3 §7.4, DDSI-RTPS 2.5 §9.6.3.8 and §10.5.
"""

from __future__ import annotations

from .encapsulation import Encoding, encapsulation_header, encapsulation_padding
from .endianness import Endianness
from .error import ImpossibleLength, NotUtf8, OutOfRange, Truncated, WireError
from .float128 import Float128, Inexact
from .reader import EmHeaderRead, PidRead, Reader, ReaderMark
# `utf8_octets` and `utf16_units` are exported because a generated codec
# names them: the shared walk emits both through
# `CodecTarget::runtime_static` where a bounded `string` or `wstring` is
# measured against its bound (§7.3.1.2.1.12.3). Without them here a
# generated module needed a second import line reaching into a submodule,
# which is a path the package's shape does not otherwise ask anyone to
# know.
from .typesupport import Discarded, Status, TryConstruct, utf8_octets, utf16_units
from .values import Mismatch, Missing, SampleError, Unknown
from .writer import (
    KEY_HASH_LEN,
    PID_EXTENDED,
    PID_IGNORE,
    PID_IMPL_EXTENSION,
    PID_LIST_END,
    PID_MUST_UNDERSTAND,
    PID_SHORT_MAX_ID,
    PID_SHORT_MAX_LEN,
    DHeader,
    EmHeader,
    ParameterRole,
    Pid,
    PlCdr1Framing,
    pid_flags_at,
    bit_pattern_bool,
    bit_pattern_f32,
    bit_pattern_f64,
    bit_pattern_int,
    no_dheader,
    Writer,
    wstring_unit_width,
)

__all__ = [
    "DHeader",
    "Float128",
    "Inexact",
    "EmHeader",
    "EmHeaderRead",
    "Encoding",
    "Endianness",
    "ImpossibleLength",
    "KEY_HASH_LEN",
    "Discarded",
    "Mismatch",
    "Missing",
    "NotUtf8",
    "OutOfRange",
    "PID_EXTENDED",
    "PID_IGNORE",
    "PID_IMPL_EXTENSION",
    "PID_LIST_END",
    "PID_MUST_UNDERSTAND",
    "PID_SHORT_MAX_ID",
    "PID_SHORT_MAX_LEN",
    "ParameterRole",
    "Pid",
    "PlCdr1Framing",
    "pid_flags_at",
    "PidRead",
    "Reader",
    "ReaderMark",
    "SampleError",
    "Status",
    "Truncated",
    "Unknown",
    "TryConstruct",
    "WireError",
    "Writer",
    "encapsulation_header",
    "encapsulation_padding",
    "bit_pattern_bool",
    "bit_pattern_f32",
    "bit_pattern_f64",
    "bit_pattern_int",
    "no_dheader",
    "utf8_octets",
    "utf16_units",
    "wstring_unit_width",
]

__version__ = "1.0.0-rc.7"
