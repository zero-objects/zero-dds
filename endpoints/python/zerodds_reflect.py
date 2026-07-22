# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# zerodds_reflect -- the wire-variable unit for the Python endpoint SDK
# (ADR 0013): a reflective XCDR codec driven by a runtime field list instead of
# generated code. Same bytes as the fixed path. Mirrors endpoints/c/*reflect*.
from __future__ import print_function

# A field is (kind, value); kind in:
#   'u8' 'u16' 'u32' 'u64' 'f32' 'f64' 'bool' 'string' 'seq_u8'


def reflect_encode(w, fields):
    for kind, value in fields:
        getattr(w, {
            'u8': 'put_u8', 'u16': 'put_u16', 'u32': 'put_u32', 'u64': 'put_u64',
            'f32': 'put_f32', 'f64': 'put_f64', 'bool': 'put_bool',
            'string': 'put_string', 'seq_u8': 'put_seq_u8',
        }[kind])(value)


def reflect_decode(r, kinds):
    out = []
    for kind in kinds:
        out.append(getattr(r, {
            'u8': 'get_u8', 'u16': 'get_u16', 'u32': 'get_u32', 'u64': 'get_u64',
            'f32': 'get_f32', 'f64': 'get_f64', 'bool': 'get_bool',
            'string': 'get_string', 'seq_u8': 'get_seq_u8',
        }[kind])())
    return out


# --- extensibility + nested (recursive struct model) ---
# A struct is a dict: {'ext': 'final'|'appendable'|'mutable',
#                      'fields': [(kind, value), ...], 'ids': [..] or None}
# kind 'nested' -> value is a sub-struct dict; 'seq_struct' -> list of dicts.

_PUT = {'u8': 'put_u8', 'u16': 'put_u16', 'u32': 'put_u32', 'u64': 'put_u64',
        'f32': 'put_f32', 'f64': 'put_f64', 'bool': 'put_bool',
        'string': 'put_string', 'seq_u8': 'put_seq_u8'}


def _encode_field(w, kind, value):
    if kind == 'nested':
        encode_struct(w, value)
    elif kind == 'seq_struct':
        bs = w.dheader_begin()
        w.put_u32(len(value))
        for e in value:
            encode_struct(w, e)
        w.dheader_end(bs)
    else:
        getattr(w, _PUT[kind])(value)


def encode_struct(w, s):
    ext = s['ext']
    fields = s['fields']
    if ext == 'final':
        for k, v in fields:
            _encode_field(w, k, v)
    elif ext == 'appendable':
        bs = w.dheader_begin()
        for k, v in fields:
            _encode_field(w, k, v)
        w.dheader_end(bs)
    elif ext == 'mutable':
        ids = s['ids']
        bs = w.dheader_begin()
        for i, (k, v) in enumerate(fields):
            e = w.emheader_begin(ids[i], 0)
            _encode_field(w, k, v)
            w.emheader_end(e)
        w.dheader_end(bs)
    else:
        raise ValueError(ext)
