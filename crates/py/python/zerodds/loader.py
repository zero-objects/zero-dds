"""ZeroDDS pure-ctypes loader per `zerodds-ffi-loader-1.0` §3.1.

This file is the canonical loader template for Python and binds
directly against `libzerodds.{so,dylib,dll}` from `crates/zerodds-c-api`.

Unlike the PyO3-based `zerodds` API, this loader is
zero-build-dep: it needs **only** the finished dynamic library and
Python's stdlib `ctypes`. This serves the 'distro package' path
(system libzerodds installed) as well as the CI pattern `cargo build -p
zerodds-c-api && python -c "from zerodds.loader import Runtime"`.

The ABI signatures follow the concrete header
`crates/zerodds-c-api/include/zerodds.h`. The spec excerpts in §2.3
show a simplified ideal form with an `out` pointer; the real
header uses *return-pointer + NULL on error* for create
functions — we bind against what is in the header.

Usage::

    from zerodds.loader import Runtime, Writer, Reader

    rt = Runtime(domain_id=42)
    w = Writer(rt, topic="Chat::Message", type_name="Chat::Message", reliable=True)
    w.wait_for_matched(1, timeout_ms=5000)
    w.write(cdr_bytes)

    r = Reader(rt, topic="Chat::Message", type_name="Chat::Message", reliable=True)
    r.wait_for_matched(1, timeout_ms=5000)
    payload = r.take()
"""
from __future__ import annotations

import ctypes
import os
import sys
from pathlib import Path
from typing import Optional

__all__ = [
    "Runtime",
    "Writer",
    "Reader",
    "DomainParticipantFactory",
    "ZeroDdsError",
    "load_library",
]


# ---------------------------------------------------------------------------
# Library-Resolution (§3.1 Loader-Pattern)
# ---------------------------------------------------------------------------


def _platform_libname() -> str:
    if sys.platform == "darwin":
        return "libzerodds.dylib"
    if sys.platform == "win32":
        return "zerodds.dll"
    return "libzerodds.so"


def load_library() -> ctypes.CDLL:
    """Load `libzerodds` via the canonical 3-Step-Resolution.

    1. ZERODDS_LIB env override (absolute path)
    2. wheel-internal `_lib/` directory
    3. system linker (`/usr/local/lib`, `LD_LIBRARY_PATH`, ...)

    Additional search paths used when called from a development tree:
    `crates/zerodds-c-api/target/debug/`,
    `target/debug/`,
    `target/release/` relative to repo-root candidates.
    """
    name = _platform_libname()

    # 1) ENV override
    env = os.environ.get("ZERODDS_LIB")
    if env:
        return ctypes.CDLL(env)

    # 2) wheel-internal lib bundle
    here = Path(__file__).resolve().parent
    bundled = here / "_lib" / name
    if bundled.exists():
        return ctypes.CDLL(str(bundled))

    # Dev-tree fallbacks (walk up looking for a workspace target)
    candidate_roots = []
    cursor = here
    for _ in range(8):
        cursor = cursor.parent
        candidate_roots.append(cursor)
    for root in candidate_roots:
        for sub in ("target/debug", "target/release"):
            cand = root / sub / name
            if cand.exists():
                return ctypes.CDLL(str(cand))

    # 3) system linker
    return ctypes.CDLL(name)


# ---------------------------------------------------------------------------
# ABI signatures (subset per crates/zerodds-c-api/include/zerodds.h)
# ---------------------------------------------------------------------------


def _bind(lib: ctypes.CDLL) -> ctypes.CDLL:
    # opaque pointers
    p_rt = ctypes.c_void_p
    p_w = ctypes.c_void_p
    p_r = ctypes.c_void_p

    # zerodds_runtime_create(uint32 domain) -> *Runtime (NULL on err)
    lib.zerodds_runtime_create.argtypes = [ctypes.c_uint32]
    lib.zerodds_runtime_create.restype = p_rt

    lib.zerodds_runtime_destroy.argtypes = [p_rt]
    lib.zerodds_runtime_destroy.restype = None

    lib.zerodds_runtime_wait_for_peers.argtypes = [
        p_rt,
        ctypes.c_int,
        ctypes.c_uint64,
    ]
    lib.zerodds_runtime_wait_for_peers.restype = ctypes.c_int

    # writer
    lib.zerodds_writer_create.argtypes = [
        p_rt,
        ctypes.c_char_p,
        ctypes.c_char_p,
        ctypes.c_int,
    ]
    lib.zerodds_writer_create.restype = p_w

    lib.zerodds_writer_write.argtypes = [
        p_w,
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.c_size_t,
    ]
    lib.zerodds_writer_write.restype = ctypes.c_int

    lib.zerodds_writer_wait_for_matched.argtypes = [
        p_w,
        ctypes.c_int,
        ctypes.c_uint64,
    ]
    lib.zerodds_writer_wait_for_matched.restype = ctypes.c_int

    lib.zerodds_writer_destroy.argtypes = [p_w]
    lib.zerodds_writer_destroy.restype = None

    # reader
    lib.zerodds_reader_create.argtypes = [
        p_rt,
        ctypes.c_char_p,
        ctypes.c_char_p,
        ctypes.c_int,
    ]
    lib.zerodds_reader_create.restype = p_r

    lib.zerodds_reader_take.argtypes = [
        p_r,
        ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)),
        ctypes.POINTER(ctypes.c_size_t),
        ctypes.POINTER(ctypes.c_uint8),  # out_repr (nullable): XCDR-Version
    ]
    lib.zerodds_reader_take.restype = ctypes.c_int

    # Endian-aware take: like zerodds_reader_take + out_be (0=little, 1=big).
    lib.zerodds_reader_take_endian.argtypes = [
        p_r,
        ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)),
        ctypes.POINTER(ctypes.c_size_t),
        ctypes.POINTER(ctypes.c_uint8),  # out_repr (nullable)
        ctypes.POINTER(ctypes.c_uint8),  # out_be (nullable): wire byte order
    ]
    lib.zerodds_reader_take_endian.restype = ctypes.c_int

    lib.zerodds_reader_wait_for_matched.argtypes = [
        p_r,
        ctypes.c_int,
        ctypes.c_uint64,
    ]
    lib.zerodds_reader_wait_for_matched.restype = ctypes.c_int

    lib.zerodds_reader_destroy.argtypes = [p_r]
    lib.zerodds_reader_destroy.restype = None

    lib.zerodds_buffer_free.argtypes = [
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.c_size_t,
    ]
    lib.zerodds_buffer_free.restype = None

    lib.zerodds_version.argtypes = []
    lib.zerodds_version.restype = ctypes.c_char_p

    # SPEC-GAP: zerodds-c-api currently exposes no `zerodds_abi_revision()` /
    # `zerodds_strerror()` / `zerodds_qos_default()` symbols even though the
    # ffi-loader-1.0 spec §2.1+§2.3 lists them. Loader works without them
    # since reliable+history are passed as direct ints to writer/reader_create.
    return lib


_lib: Optional[ctypes.CDLL] = None


def _get_lib() -> ctypes.CDLL:
    global _lib
    if _lib is None:
        _lib = _bind(load_library())
    return _lib


# ---------------------------------------------------------------------------
# Pythonic wrappers
# ---------------------------------------------------------------------------


class ZeroDdsError(RuntimeError):
    """Raised when an FFI call returns a negative status or NULL pointer."""


class Runtime:
    """Owns a ZeroDDS runtime + implicit DomainParticipant on `domain_id`."""

    def __init__(self, domain_id: int = 0) -> None:
        lib = _get_lib()
        ptr = lib.zerodds_runtime_create(ctypes.c_uint32(domain_id))
        if not ptr:
            raise ZeroDdsError(
                f"zerodds_runtime_create returned NULL for domain={domain_id}"
            )
        self._lib = lib
        self._ptr = ctypes.c_void_p(ptr)
        self._domain_id = domain_id

    @property
    def domain_id(self) -> int:
        return self._domain_id

    @property
    def raw(self) -> ctypes.c_void_p:
        return self._ptr

    def wait_for_peers(self, min_count: int, timeout_ms: int) -> int:
        rc = self._lib.zerodds_runtime_wait_for_peers(
            self._ptr, ctypes.c_int(min_count), ctypes.c_uint64(timeout_ms)
        )
        return int(rc)

    def close(self) -> None:
        if self._ptr:
            self._lib.zerodds_runtime_destroy(self._ptr)
            self._ptr = ctypes.c_void_p()

    def __enter__(self) -> "Runtime":
        return self

    def __exit__(self, *exc) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:  # pragma: no cover - best-effort finalizer
            pass


class Writer:
    def __init__(
        self,
        runtime: Runtime,
        topic: str,
        type_name: Optional[str] = None,
        reliable: bool = True,
    ) -> None:
        if type_name is None:
            type_name = topic
        lib = runtime._lib
        ptr = lib.zerodds_writer_create(
            runtime._ptr,
            topic.encode("utf-8"),
            type_name.encode("utf-8"),
            ctypes.c_int(1 if reliable else 0),
        )
        if not ptr:
            raise ZeroDdsError(
                f"zerodds_writer_create failed for topic={topic!r}"
            )
        self._lib = lib
        self._ptr = ctypes.c_void_p(ptr)
        self.topic = topic

    def write(self, payload: bytes) -> None:
        buf_t = ctypes.c_uint8 * len(payload)
        buf = buf_t.from_buffer_copy(payload)
        rc = self._lib.zerodds_writer_write(
            self._ptr,
            ctypes.cast(buf, ctypes.POINTER(ctypes.c_uint8)),
            ctypes.c_size_t(len(payload)),
        )
        if rc != 0:
            raise ZeroDdsError(f"zerodds_writer_write rc={rc}")

    def wait_for_matched(self, min_count: int = 1, timeout_ms: int = 5000) -> None:
        rc = self._lib.zerodds_writer_wait_for_matched(
            self._ptr, ctypes.c_int(min_count), ctypes.c_uint64(timeout_ms)
        )
        if rc != 0:
            raise ZeroDdsError(
                f"writer wait_for_matched(min={min_count}, "
                f"timeout_ms={timeout_ms}) rc={rc}"
            )

    def close(self) -> None:
        if self._ptr:
            self._lib.zerodds_writer_destroy(self._ptr)
            self._ptr = ctypes.c_void_p()

    def __enter__(self) -> "Writer":
        return self

    def __exit__(self, *exc) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:  # pragma: no cover
            pass


class Reader:
    def __init__(
        self,
        runtime: Runtime,
        topic: str,
        type_name: Optional[str] = None,
        reliable: bool = True,
    ) -> None:
        if type_name is None:
            type_name = topic
        lib = runtime._lib
        ptr = lib.zerodds_reader_create(
            runtime._ptr,
            topic.encode("utf-8"),
            type_name.encode("utf-8"),
            ctypes.c_int(1 if reliable else 0),
        )
        if not ptr:
            raise ZeroDdsError(
                f"zerodds_reader_create failed for topic={topic!r}"
            )
        self._lib = lib
        self._ptr = ctypes.c_void_p(ptr)
        self.topic = topic

    def take(self) -> Optional[bytes]:
        """Take a single sample. Returns bytes or None if no sample ready."""
        out_buf = ctypes.POINTER(ctypes.c_uint8)()
        out_len = ctypes.c_size_t(0)
        # out_repr = None (NULL): the XCDR version is not needed here.
        # The fourth argument MUST be passed — otherwise
        # the C function reads it from an uninitialized register
        # and writes `repr` to a garbage address (SIGSEGV).
        rc = self._lib.zerodds_reader_take(
            self._ptr, ctypes.byref(out_buf), ctypes.byref(out_len), None
        )
        if rc != 0:
            raise ZeroDdsError(f"zerodds_reader_take rc={rc}")
        if not out_buf or out_len.value == 0:
            return None
        try:
            return bytes(
                ctypes.cast(out_buf, ctypes.POINTER(ctypes.c_uint8 * out_len.value))[0]
            )
        finally:
            self._lib.zerodds_buffer_free(out_buf, out_len)

    def take_endian(self) -> "Optional[tuple[bytes, bool, int]]":
        """Take a single sample as ``(bytes, big_endian, representation)``.
        ``big_endian`` is the wire byte order from the encapsulation header
        (RTPS 2.5 §10.5) and ``representation`` is the XCDR version (``0`` =
        XCDR1 / classic CDR, ``1`` = XCDR2), so a typed consumer can pick
        ``decode(.., endian, representation)`` and read a big-endian and/or
        XCDR1 peer correctly. ``None`` if no sample is ready."""
        out_buf = ctypes.POINTER(ctypes.c_uint8)()
        out_len = ctypes.c_size_t(0)
        out_be = ctypes.c_uint8(0)
        out_repr = ctypes.c_uint8(1)
        rc = self._lib.zerodds_reader_take_endian(
            self._ptr,
            ctypes.byref(out_buf),
            ctypes.byref(out_len),
            ctypes.byref(out_repr),
            ctypes.byref(out_be),
        )
        if rc != 0:
            raise ZeroDdsError(f"zerodds_reader_take_endian rc={rc}")
        if not out_buf or out_len.value == 0:
            return None
        try:
            data = bytes(
                ctypes.cast(out_buf, ctypes.POINTER(ctypes.c_uint8 * out_len.value))[0]
            )
        finally:
            self._lib.zerodds_buffer_free(out_buf, out_len)
        return data, bool(out_be.value), int(out_repr.value)

    def take_all(self, max_samples: int = 16) -> list[bytes]:
        out: list[bytes] = []
        for _ in range(max_samples):
            sample = self.take()
            if sample is None:
                break
            out.append(sample)
        return out

    def wait_for_matched(self, min_count: int = 1, timeout_ms: int = 5000) -> None:
        rc = self._lib.zerodds_reader_wait_for_matched(
            self._ptr, ctypes.c_int(min_count), ctypes.c_uint64(timeout_ms)
        )
        if rc != 0:
            raise ZeroDdsError(
                f"reader wait_for_matched(min={min_count}, "
                f"timeout_ms={timeout_ms}) rc={rc}"
            )

    def close(self) -> None:
        if self._ptr:
            self._lib.zerodds_reader_destroy(self._ptr)
            self._ptr = ctypes.c_void_p()

    def __enter__(self) -> "Reader":
        return self

    def __exit__(self, *exc) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:  # pragma: no cover
            pass


class DomainParticipantFactory:
    """Spec-flavoured factory shim around `Runtime`.

    ZeroDDS's C-ABI today fuses Factory+Participant into one
    `zerodds_runtime_create(domain_id)` call. This thin shim keeps the
    DDS §2.2.2 idiom (`factory.create_participant(domain_id)` returns a
    Participant-like) so port-code that mirrors RTI/Cyclone-shape stays
    portable.
    """

    @classmethod
    def instance(cls) -> "DomainParticipantFactory":
        return cls()

    def create_participant(self, domain_id: int = 0) -> Runtime:
        return Runtime(domain_id=domain_id)
