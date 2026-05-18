"""Tests fuer §6.6 — ReadCondition + QueryCondition + State-Bitmask-Konstanten.

Verifiziert Konstruktion, Attach an WaitSet, und SQL-Filter-Validierung
fuer QueryCondition.
"""

from __future__ import annotations

import pytest

import zerodds

pytestmark = pytest.mark.skipif(
    not getattr(zerodds, "_CORE_AVAILABLE", False),
    reason="zerodds._core nicht kompiliert — maturin develop noetig",
)


# ---------------------------------------------------------------------------
# State-Bitmask-Konstanten (§6.6, DDS 1.4 §2.2.2.5.1.4)
# ---------------------------------------------------------------------------


def test_sample_state_constants():
    assert zerodds.sample_state.NOT_READ == 1
    assert zerodds.sample_state.READ == 2
    assert zerodds.sample_state.ANY == 3


def test_view_state_constants():
    assert zerodds.view_state.NEW == 1
    assert zerodds.view_state.NOT_NEW == 2
    assert zerodds.view_state.ANY == 3


def test_instance_state_constants():
    assert zerodds.instance_state.ALIVE == 1
    assert zerodds.instance_state.NOT_ALIVE_DISPOSED == 2
    assert zerodds.instance_state.NOT_ALIVE_NO_WRITERS == 4
    assert zerodds.instance_state.NOT_ALIVE == 6
    assert zerodds.instance_state.ANY == 7


# ---------------------------------------------------------------------------
# ReadCondition
# ---------------------------------------------------------------------------


def test_read_condition_constructible():
    """§6.6 — ReadCondition mit drei Bitmasks."""
    rc = zerodds.ReadCondition(
        zerodds.sample_state.ANY,
        zerodds.view_state.ANY,
        zerodds.instance_state.ALIVE,
    )
    assert rc.get_sample_state_mask() == 3
    assert rc.get_view_state_mask() == 3
    assert rc.get_instance_state_mask() == 1


def test_read_condition_modes():
    """§6.6 — state_check_mode='any'|'always'|'never' beeinflusst Trigger."""
    rc_any = zerodds.ReadCondition(3, 3, 7, "any")
    assert rc_any.get_trigger_value() is True

    rc_always = zerodds.ReadCondition(0, 0, 0, "always")
    assert rc_always.get_trigger_value() is True

    rc_never = zerodds.ReadCondition(3, 3, 7, "never")
    assert rc_never.get_trigger_value() is False


def test_read_condition_unknown_mode_raises():
    with pytest.raises(RuntimeError, match="state_check_mode"):
        zerodds.ReadCondition(3, 3, 7, "elsewhere")


def test_read_condition_attaches_to_waitset():
    rc = zerodds.ReadCondition(
        zerodds.sample_state.NOT_READ,
        zerodds.view_state.NEW,
        zerodds.instance_state.ALIVE,
    )
    ws = zerodds.WaitSet()
    ws.attach_read_condition(rc)
    # WaitSet.wait wirft TimeoutError, weil rc.trigger == "any"-Mode
    # in offline-Pfad nicht greift; wir testen nur, dass attach klappt.


# ---------------------------------------------------------------------------
# QueryCondition
# ---------------------------------------------------------------------------


def test_query_condition_constructible_simple_filter():
    """§6.6 — QueryCondition mit gueltigem SQL92-Filter."""
    qc = zerodds.QueryCondition(
        zerodds.sample_state.ANY,
        zerodds.view_state.ANY,
        zerodds.instance_state.ALIVE,
        "x > 5",
        [],
    )
    assert qc is not None


def test_query_condition_with_parameters():
    """§6.6 — QueryCondition akzeptiert positional Query-Parameter (%0, %1, ...)."""
    qc = zerodds.QueryCondition(
        zerodds.sample_state.ANY,
        zerodds.view_state.ANY,
        zerodds.instance_state.ANY,
        "color = %0 AND x > %1",
        ["RED", "10"],
    )
    assert qc is not None


def test_query_condition_invalid_sql_raises():
    """§6.6 — Ungueltige SQL-Expression → RuntimeError im Konstruktor."""
    with pytest.raises(RuntimeError, match="QueryCondition"):
        zerodds.QueryCondition(
            zerodds.sample_state.ANY,
            zerodds.view_state.ANY,
            zerodds.instance_state.ANY,
            "this is not (valid SQL",
            [],
        )


def test_query_condition_attaches_to_waitset():
    qc = zerodds.QueryCondition(
        zerodds.sample_state.ANY,
        zerodds.view_state.ANY,
        zerodds.instance_state.ALIVE,
        "x < 100",
        [],
    )
    ws = zerodds.WaitSet()
    ws.attach_query_condition(qc)
