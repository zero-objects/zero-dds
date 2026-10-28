"""`zerodds.loader.Writer` declares the type's extensibility to the C ABI.

The encapsulation identifier the runtime stamps on a user payload is
chosen from the writer slot's `wire_extensibility`, which defaults to
FINAL — `PLAIN_CDR2_LE`, `0x0007`. A generated `@appendable` body has a
leading DHEADER and a `@mutable` one an EMHEADER per member, so a writer
that never calls `zerodds_writer_set_wire_extensibility` announces
`0x0007` over a body that is neither. Measured on the Linux test host against OpenDDS 3.35.0, three
topics whose types differ in nothing but the extensibility, 24 samples
each::

    body ZdExtKFinal       07b201000e640300
    body ZdExtKAppendable  0800000007b201000e640300
    body ZdExtKMutable     100000000100002007b20100020000200e640300

    SAMPLES ZdExtKFinal       24     <- the control
    SAMPLES ZdExtKAppendable   0
    SAMPLES ZdExtKMutable      0

    ERROR: to_encoding_i: expected appendable extensibility, but got
           XCDR2 Little Endian Plain                            (x24)
    ERROR: to_encoding_i: expected mutable extensibility, but got
           XCDR2 Little Endian Plain                            (x24)

and all three bodies left with `00070000` in front of them. CycloneDDS
decodes all three regardless — it reads the body with its own type and
not with the header — which is what made the defect invisible between
two lenient peers.

# Why a recording stub and not a live topic

What is under test is one seam: `Writer.__init__` hands the type's
extensibility to the C ABI. A live pub/sub run would need a domain,
discovery and a reader to observe a value this test can read straight
off the call, and would not run at all where `libzerodds` is not built —
which is a skip, and a skipped test reports the same "ok" as one that
ran. So the C ABI is a recorder here: nothing is loaded, nothing is
published, and the recorded argument is the assertion. The mapping from
that argument to the four octets on the wire is
`crates/dcps/src/runtime.rs::user_payload_encap`.
"""

from __future__ import annotations


import pytest

from zerodds import loader

from generated_modules import load_generated

#: Same numbering as a generated `TopicDescriptor.extensibility` int.
_EXTENSIBILITY_NAME = {0: "final", 1: "appendable", 2: "mutable"}


class _RecordingAbi:
    """The C ABI, as far as `Writer.__init__` reaches it.

    Every create returns a distinct truthy handle so a call landing on
    the wrong writer would be visible, and the path each writer took is
    recorded next to the extensibility it declared.
    """

    def __init__(self, set_rc: int = 0) -> None:
        self.set_rc = set_rc
        self.calls: list[tuple[str, int]] = []
        self.destroyed: list[int] = []
        self._next_handle = 0x1000
        self._path = "none"

    # -- runtime ---------------------------------------------------------
    def zerodds_runtime_create(self, _domain):  # noqa: D102
        return 0x0FFF

    def zerodds_runtime_destroy(self, _rt) -> None:  # noqa: D102
        return None

    # -- writer ----------------------------------------------------------
    def _handle(self) -> int:
        self._next_handle += 1
        return self._next_handle

    def zerodds_writer_create(self, *_args):  # noqa: D102
        self._path = "create"
        return self._handle()

    def zerodds_writer_create_typed(self, *_args):  # noqa: D102
        self._path = "typed"
        return self._handle()

    def zerodds_writer_set_wire_extensibility(self, _writer, extensibility):
        """Records the declared extensibility and the arm it came from."""
        self.calls.append((self._path, int(extensibility.value)))
        return self.set_rc

    def zerodds_writer_destroy(self, writer) -> None:
        """`Writer.close` hands a `c_void_p`, the failure path a raw int."""
        self.destroyed.append(int(getattr(writer, "value", writer) or 0))


@pytest.fixture()
def abi(monkeypatch: pytest.MonkeyPatch) -> _RecordingAbi:
    recorder = _RecordingAbi()
    monkeypatch.setattr(loader, "_get_lib", lambda: recorder)
    return recorder


def _writer(rt: loader.Runtime, name: str, ext: str, *, typed: bool) -> loader.Writer:
    return loader.Writer(
        rt,
        topic=name,
        type_name=name,
        # A `type_object` is what routes the create to
        # `zerodds_writer_create_typed`; without one it takes the
        # byte-oriented `zerodds_writer_create`.
        type_object=b"\x50\x00\x02\x00" if typed else None,
        extensibility=ext,
    )


def test_writer_declares_the_types_extensibility_on_both_create_paths(
    abi: _RecordingAbi,
) -> None:
    """`0`, `1` and `2` reach the C ABI for final, appendable and mutable.

    Both arms are asserted: a call placed in the `type_object` arm alone
    would leave every type without a TypeObject — every union, every type
    XTypes cannot lower — announcing FINAL, and the third row here is the
    one that would catch it.
    """
    rt = loader.Runtime(domain_id=0)
    # Held, not dropped: a writer collected mid-test would run `close()`
    # and put a destroy into the recorder the last assertion reads.
    live = [
        _writer(rt, "Fin", "final", typed=True),
        _writer(rt, "App", "appendable", typed=True),
        _writer(rt, "Mut", "mutable", typed=False),
    ]

    assert abi.calls == [("typed", 0), ("typed", 1), ("create", 2)]
    assert abi.destroyed == []
    assert len(live) == 3


def test_extensibility_comes_from_the_generated_descriptor(abi: _RecordingAbi) -> None:
    """The value a caller passes is the one the generated `TopicDescriptor`
    recorded (`layout.idl`'s `x::App`, `@appendable`).

    `TopicDescriptor.extensibility` (an int: 0 final, 1 appendable, 2
    mutable) reads the framing decision off the class the codegen built,
    so the header the writer announces and the body `encode()` produced
    come from one source and cannot drift apart.
    """
    mod = load_generated("layout_gen")
    descriptor = mod.ZD_TOPIC_DESCRIPTORS["x::App"]
    assert descriptor.extensibility == 1

    rt = loader.Runtime(domain_id=0)
    live = _writer(
        rt, "x::App", _EXTENSIBILITY_NAME[descriptor.extensibility], typed=True,
    )

    assert abi.calls == [("typed", 1)]
    assert live is not None


def test_a_writer_that_cannot_declare_its_framing_is_not_handed_out(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A refused declaration destroys the writer and raises.

    Keeping it would publish samples under an identifier that does not
    name their framing — the silent drop this call exists to end.
    """
    recorder = _RecordingAbi(set_rc=-7)
    monkeypatch.setattr(loader, "_get_lib", lambda: recorder)
    rt = loader.Runtime(domain_id=0)

    with pytest.raises(loader.ZeroDdsError, match="set_wire_extensibility"):
        _writer(rt, "App", "appendable", typed=True)

    assert recorder.destroyed == [0x1001]


def test_an_unknown_extensibility_never_reaches_the_c_abi(abi: _RecordingAbi) -> None:
    """A misspelt extensibility is rejected before the writer exists.

    Silently reading it as FINAL is exactly the value this parameter
    exists to stop being guessed.
    """
    rt = loader.Runtime(domain_id=0)
    with pytest.raises(ValueError, match="appendable"):
        _writer(rt, "Typo", "apendable", typed=True)
    assert abi.calls == []
